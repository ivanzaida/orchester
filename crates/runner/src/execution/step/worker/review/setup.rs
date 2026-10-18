//! The project's reviewer, resolved when a review starts (`orchester_core::run::review::resolve_reviewer`):
//! a CLI (Claude Code or Codex) run in an empty folder of its own, or a native connection's client with
//! the key read from the keychain. Without a reviewer, or with one that cannot be reached, the review
//! fails closed and the person decides; the hard rules still come first either way. A review that asked a
//! model records what it cost ([`ReviewLedger`]).

use std::{
  path::{Path, PathBuf},
  time::Duration,
};

use orchester_agent::AnthropicClient;
use orchester_core::{
  connection::{
    AgentProvider, ApiEndpoint, ConnectionKind,
    invocation::{Platform, account_environment, agent_environment},
    reviewer::{ClaudeReviewer, CodexReviewer},
  },
  id::{AgentAccountId, SecretId},
  permission::{AgentPermissions, RepositoryAccess, category::CommandPart},
  resource::ModelId,
  run::{
    ApprovalSubject,
    review::{
      AutoReview, HoldRules, PushFacts, RequestReviewer, ResolvedReviewer, Review, ReviewError, ReviewHold,
      ReviewInput, ReviewSpend, Reviewed, ReviewerKind, resolve_reviewer, review_hold, review_request,
    },
  },
  secret::{SecretValue, SecretVault},
  store::{ConnectionStore, SecretStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::StoreError;
use tokio::{runtime::Handle, sync::watch};

use super::{
  super::{Watch, Worker},
  ledger::ReviewLedger,
  native::{NativeReviewer, client},
  process::{ReviewPlace, SupervisedRunner},
};
use crate::git::Git;

/// The project's reviewer, before one review runs.
pub(super) enum ReviewerSetup {
  /// Claude Code or Codex, by its program.
  Cli {
    kind: ReviewerKind,
    executable: PathBuf,
    /// The account of the reviewer's connection, whose plan limits it reports.
    account: Option<AgentAccountId>,
    model: ModelId,
    effort: Option<String>,
    environment: Vec<(String, String)>,
  },
  /// A native connection's client.
  Native {
    client: AnthropicClient,
    model: ModelId,
    effort: Option<String>,
  },
  /// A reviewer that cannot be reached (its key could not be read): every review fails.
  Unreachable { model: ModelId },
  /// The project has no reviewer.
  Missing,
}

/// Where a push of the run would go: its branch, its worktree, and the push URL of `origin` recorded as it
/// started.
pub(super) struct PushPlace {
  pub(super) branch: Option<String>,
  pub(super) worktree: PathBuf,
  pub(super) origin: Option<String>,
}

impl PushPlace {
  /// The branch checked out in the worktree now, and whether `origin` still pushes where it did as the
  /// run started; what git cannot tell counts against the push.
  pub(super) fn read(&self) -> (Option<String>, bool) {
    let git = Git::new(&self.worktree);
    let answer = |args: &[&str]| match git.attempt(args) {
      Ok(Ok(out)) => Some(out),
      Ok(Err(_)) => None,
      Err(error) => {
        tracing::warn!(%error, "cannot ask git where a push would go");
        None
      }
    };
    let head = answer(&["symbolic-ref", "--quiet", "--short", "HEAD"]);
    let origin = answer(&["remote", "get-url", "--push", "origin"]);
    let kept = matches!((&origin, &self.origin), (Some(now), Some(then)) if now == then);
    (head, kept)
  }
}

/// The target directories of `subject`'s Cargo builds in `worktree` that its repository ignores and tracks no file in
/// (`HoldRules::build_output`), asked of git only in a step that reads the repository only, where it matters.
pub(super) fn ignored_build_output(
  worktree: &Path,
  subject: &ApprovalSubject,
  permissions: Option<&AgentPermissions>,
) -> Vec<PathBuf> {
  let ApprovalSubject::Command(command) = subject else {
    return Vec::new();
  };
  if permissions.is_none_or(|permissions| permissions.repository() != RepositoryAccess::ReadOnly) {
    return Vec::new();
  }
  let git = Git::new(worktree);
  let mut ignored: Vec<PathBuf> = Vec::new();
  for output in command.parts().iter().filter_map(CommandPart::build_output) {
    let Some(relative) = output.in_worktree() else {
      continue;
    };
    if ignored.iter().any(|folder| folder == output.folder()) {
      continue;
    }
    // Git's own spelling of the folder: forward slashes, a trailing one for a folder.
    let folder = format!("{}/", relative.to_string_lossy().replace('\\', "/"));
    let asked = git
      .answer(&["check-ignore", "-q", "--", &folder])
      .and_then(|ignores| Ok(ignores && git.stdout(&["ls-files", "--", &folder])?.is_empty()));
    match asked {
      Ok(true) => ignored.push(output.folder().to_path_buf()),
      Ok(false) => {}
      // A build whose folder git cannot tell about holds, as one it does not ignore.
      Err(error) => tracing::warn!(%error, "cannot ask git whether a build's target directory is ignored"),
    }
  }
  ignored
}

/// What a review of one request needs besides the reviewer.
pub(super) struct ReviewJob {
  pub(super) subject: ApprovalSubject,
  pub(super) permissions: Option<AgentPermissions>,
  /// What tells where a push of the run would go.
  pub(super) push: PushPlace,
  /// The build output folders the worktree's repository ignores ([`ignored_build_output`]).
  pub(super) ignored: Vec<PathBuf>,
  /// Codex asks to run the command outside its sandbox.
  pub(super) escalation: bool,
  pub(super) input: Result<ReviewInput, ReviewHold>,
  pub(super) runtime: Handle,
  pub(super) timeout: Duration,
  pub(super) stop: watch::Receiver<()>,
}

/// The hard rule that keeps `subject` from the reviewer, or its input's (a request too long to review
/// whole); checked before the review starts, so a held request asks the person at once.
pub(super) fn hold_now(
  (subject, permissions, escalation): (&ApprovalSubject, Option<&AgentPermissions>, bool),
  (push, ignored): (&PushPlace, &[PathBuf]),
  input: &Result<ReviewInput, ReviewHold>,
) -> Option<ReviewHold> {
  let (head, origin_kept) = push.read();
  let rules = HoldRules {
    permissions,
    push: PushFacts {
      branch: push.branch.as_deref(),
      head: head.as_deref(),
      origin_kept,
    },
    escalation,
    build_output: ignored,
  };
  review_hold(subject, &rules).or_else(|| input.as_ref().err().copied())
}

impl ReviewerSetup {
  /// Auto mode's handling of `job`'s request with this reviewer, from the blocking pool; what it cost goes
  /// to `ledger` once it ends.
  pub(super) fn review(self, job: ReviewJob, ledger: &ReviewLedger) -> Review {
    let started_at = Timestamp::now();
    let provider = self.provider();
    let done = self.judge(job);
    if let Some(spent) = provider {
      ledger.record(spent, started_at, &done);
    }
    done.review
  }

  /// The provider and account a review with this reviewer is spent on; `None` when it asks no model.
  const fn provider(&self) -> Option<(AgentProvider, Option<AgentAccountId>)> {
    match self {
      Self::Cli { kind, account, .. } => Some((kind.provider(), *account)),
      Self::Native { .. } => Some((AgentProvider::AnthropicApi, None)),
      Self::Unreachable { .. } | Self::Missing => None,
    }
  }

  /// The review itself, and what it cost when it asked a model.
  fn judge(self, job: ReviewJob) -> AutoReview {
    let ReviewJob {
      subject,
      permissions,
      push,
      ignored,
      escalation,
      input,
      runtime,
      timeout,
      stop,
    } = job;
    let (head, origin_kept) = push.read();
    let hold_rules = HoldRules {
      permissions: permissions.as_ref(),
      push: PushFacts {
        branch: push.branch.as_deref(),
        head: head.as_deref(),
        origin_kept,
      },
      escalation,
      build_output: &ignored,
    };
    let rules = (&subject, &hold_rules);
    match self {
      Self::Missing => review_request(None, rules, input),
      Self::Unreachable { model } => review_request(Some(&Unreachable { model }), rules, input),
      Self::Native { client, model, effort } => {
        let reviewer = NativeReviewer {
          runtime,
          client,
          model,
          effort,
          timeout,
          stop,
        };
        review_request(Some(&reviewer), rules, input)
      }
      Self::Cli {
        kind,
        executable,
        account: _,
        model,
        effort,
        environment,
      } => {
        let place = match ReviewPlace::new() {
          Ok(place) => place,
          Err(error) => {
            tracing::warn!(%error, "cannot make the folder a review runs in");
            return review_request(Some(&Unreachable { model }), rules, input);
          }
        };
        let runner = SupervisedRunner { runtime, timeout, stop };
        let folder = (place.folder(), environment);
        let reviewer: Box<dyn RequestReviewer> = match kind {
          ReviewerKind::Codex => Box::new(CodexReviewer::new(&executable, (model, effort), folder, runner)),
          _ => Box::new(ClaudeReviewer::new(&executable, (model, effort), folder, runner)),
        };
        let done = review_request(Some(reviewer.as_ref()), rules, input);
        // The folder goes only after the reviewer, which runs in it, has ended.
        drop(place);
        done
      }
    }
  }
}

/// A reviewer that cannot be reached: each review fails, and the person decides.
struct Unreachable {
  model: ModelId,
}

impl RequestReviewer for Unreachable {
  fn model(&self) -> &ModelId {
    &self.model
  }

  fn review(&self, _: &ReviewInput) -> Reviewed {
    let unreachable = ReviewError::CouldNotRun("the reviewer cannot be reached".to_owned());
    Reviewed::new(Err(unreachable), ReviewSpend::Nothing)
  }
}

impl Worker {
  /// The project's reviewer, as its auto mode settings resolve against its connections.
  pub(super) fn reviewer(&self, watch: &Watch) -> Result<ReviewerSetup, StoreError> {
    let settings = watch.db.auto_mode_settings()?;
    let connections = watch.db.connections()?;
    let Ok(resolved) = resolve_reviewer(&settings, &connections) else {
      return Ok(ReviewerSetup::Missing);
    };
    let ResolvedReviewer {
      connection,
      kind,
      model,
      effort,
      ..
    } = resolved;
    if let (ReviewerKind::AnthropicApi, Some(api)) = (kind, connection.endpoint().api()) {
      return Ok(self.native(watch, api, (model, effort)));
    }
    let Some(executable) = connection.executable() else {
      return Ok(ReviewerSetup::Missing);
    };
    let ConnectionKind::Agent(provider) = *connection.kind() else {
      return Ok(ReviewerSetup::Missing);
    };
    if let Some(reason) = connection
      .account()
      .start_problem(provider, Platform::current(), Path::is_dir)
    {
      tracing::warn!(%reason, "the reviewer's connection cannot run on its account");
      return Ok(ReviewerSetup::Missing);
    }
    let mut environment = agent_environment(std::env::vars_os());
    watch.db.project_environment()?.add_to(&mut environment);
    let environment = match account_environment(environment, connection.account(), provider) {
      Ok(environment) => environment,
      Err(error) => {
        tracing::warn!(%error, "the reviewer's account folder cannot reach its CLI");
        return Ok(ReviewerSetup::Missing);
      }
    };
    Ok(ReviewerSetup::Cli {
      kind,
      executable: executable.to_path_buf(),
      account: connection.account().id(provider),
      model,
      effort,
      environment,
    })
  }

  /// The client of a native reviewer at `api`; unreachable when its key cannot be read.
  fn native(&self, watch: &Watch, api: &ApiEndpoint, (model, effort): (ModelId, Option<String>)) -> ReviewerSetup {
    let key = match self.api_key(watch, api.key()) {
      Some(key) => key,
      None => return ReviewerSetup::Unreachable { model },
    };
    match client(api, key) {
      Ok(client) => ReviewerSetup::Native { client, model, effort },
      Err(error) => {
        tracing::warn!(%error, "cannot make the native reviewer's client");
        ReviewerSetup::Unreachable { model }
      }
    }
  }

  /// The value of the project secret `id` holding a native reviewer's API key; why not is logged, never
  /// the value.
  fn api_key(&self, watch: &Watch, id: SecretId) -> Option<SecretValue> {
    let secret = match watch.db.secret(id) {
      Ok(Some(secret)) if !secret.is_archived() => secret,
      Ok(_) => {
        tracing::warn!(%id, "the native reviewer's API key is missing or archived");
        return None;
      }
      Err(error) => {
        tracing::warn!(%id, %error, "cannot read the native reviewer's API key");
        return None;
      }
    };
    self
      .reviewer_keys
      .vault
      .read(&secret.vault_key(self.reviewer_keys.project))
      .inspect_err(|error| tracing::warn!(%id, %error, "the keychain did not give the native reviewer's API key"))
      // `.ok()`: the error is logged above, and a reviewer without its key fails every review.
      .ok()
  }
}

/// Where a native reviewer's key is read: the keychain, and the project its secrets belong to.
pub(in crate::execution::step) struct ReviewerKeys {
  pub(in crate::execution::step) vault: std::sync::Arc<dyn SecretVault>,
  pub(in crate::execution::step) project: orchester_core::id::ProjectId,
}
