//! Manual's reviewer end to end with `orchester-fake-claude` as both the agent and the reviewer: in Manual, a command
//! the rules leave to a person is approved by the reviewer without the person, in an empty folder of its own
//! with its system prompt, an unknown command too, also in a step that only reads (ORC-339), and a plain push of the
//! run's branch to the origin recorded at its start, and a Cargo test in a step that only reads whose output git
//! ignores; a credential read, a force push, or a test whose output git does not ignore in a
//! step that only reads, and every Codex escalation still ask the person without a review; a question of the
//! reviewer and an answer it cannot read leave the request to the person with what the reviewer said; a run switched
//! to Manual during a review keeps it; and a reviewer still at work when the person answers is stopped. A Codex
//! connection reviews through `codex exec` in its read-only sandbox (`orchester-fake-codex` as the reviewer). Needs
//! `git`. Run with `cargo test -p orchester-runner --features test-agent --test auto_mode auto_mode`.

mod support;

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  id::{IdempotencyKey, RunId},
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::{CommandEnvironment, ModelId, VariableName},
  run::{
    Answerer, ApprovalAnswer, DecidedBy, EffectiveMode, ManualBecause, ManualSwitch, PendingToolApproval, Run,
    review::{AutoModeSettings, Review, ReviewFailure, ReviewHold, ReviewerChoice},
  },
  store::{ActivityStore, ConnectionStore, SettingsStore},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tempfile::TempDir;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, is_running, project, scheduler_with, wait_for};

/// A Claude Code Builder that asks about every command, in a writing step with `network`.
fn asking(brief: &'static str, network: NetworkAccess) -> Setup {
  Setup {
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AskBefore,
      network,
    )),
    ..asking_in(brief, RepositoryAccess::Write)
  }
}

/// A Claude Code Builder that asks about every command, in a step with `repository` access.
fn asking_in(brief: &'static str, repository: RepositoryAccess) -> Setup {
  Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(AgentPermissions::new(
      repository,
      CommandPermission::AskBefore,
      NetworkAccess::On,
    )),
    ..Setup::new(brief)
  }
}

/// A Claude Code Builder that runs `git` and asks about every other command.
fn claude(brief: &'static str) -> Setup {
  let listed = AllowedCommands::new(vec![CommandPrefix::new("git").expect("prefix")]).expect("list");
  Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(listed),
      NetworkAccess::On,
    )),
    ..Setup::new(brief)
  }
}

struct Started {
  project: TestProject,
  scheduler: Scheduler,
  events: mpsc::UnboundedReceiver<Event>,
  id: RunId,
  /// Where the fake reviewer writes where it ran.
  log: TempDir,
}

impl Started {
  /// A run of `setup` in Manual, with the default reviewer.
  async fn new(setup: &Setup) -> Self {
    Self::of(project(setup), None).await
  }

  /// A run of `project`'s task in Manual, which `reviewer` reviews.
  async fn of(project: TestProject, reviewer: Option<ReviewerChoice>) -> Self {
    let log = tempfile::tempdir().expect("a folder for the review log");
    let mut db = ProjectDb::open(project.repo.path()).expect("open");
    let settings = AutoModeSettings { reviewer };
    db.save_auto_mode_settings(&settings).expect("the reviewer");
    let variable = VariableName::new("FAKE_REVIEW_LOG").expect("name");
    let path = log.path().join("review.log").display().to_string();
    let environment = CommandEnvironment::new(BTreeMap::from([(variable, path)])).expect("environment");
    db.save_project_environment(&environment).expect("project environment");
    let (scheduler, events) = scheduler_with(project.repo.path(), project.vault.clone());
    let id = scheduler
      .start_run(IdempotencyKey::generate(), project.task)
      .await
      .expect("the run starts");
    Self {
      project,
      scheduler,
      events,
      id,
      log,
    }
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.project.repo.path()).expect("open")
  }

  /// The first waiting request, once auto mode has left it to the person.
  async fn left_to_person(&mut self) -> (PendingToolApproval, Review) {
    let root = self.project.repo.path().to_path_buf();
    let (id, events) = (self.id, &mut self.events);
    tokio::time::timeout(PATIENCE, async {
      loop {
        let db = ProjectDb::open(&root).expect("open");
        if let Some(request) = db.pending_tool_approvals(id).expect("requests").first()
          && let Some(review) = db.tool_approval_review(id, &request.id).expect("review")
        {
          return (request.clone(), review);
        }
        events.recv().await.expect("notification");
      }
    })
    .await
    .expect("auto mode leaves a request to the person")
  }

  /// The first request auto mode's reviewer is judging: it waits, but not yet for the person.
  async fn under_review(&mut self) -> PendingToolApproval {
    let root = self.project.repo.path().to_path_buf();
    let (id, events) = (self.id, &mut self.events);
    tokio::time::timeout(PATIENCE, async {
      loop {
        let db = ProjectDb::open(&root).expect("open");
        let reviewing = db.reviewing_tool_approvals().expect("under review");
        if let Some(request) = db
          .pending_tool_approvals(id)
          .expect("requests")
          .into_iter()
          .find(|request| reviewing.contains(&request.id))
        {
          return request;
        }
        events.recv().await.expect("notification");
      }
    })
    .await
    .expect("a request under review")
  }

  /// Whether the run's agent waits for the person, as the run's phase and *Needs you* read it.
  fn waits_for_person(&self) -> bool {
    self
      .db()
      .invocations(self.id)
      .expect("invocations")
      .iter()
      .any(|invocation| invocation.timing.waiting_since().is_some())
  }

  async fn ended(&mut self) -> Run {
    wait_for(self.project.repo.path(), &mut self.events, self.id, |run| {
      run.status().is_terminal()
    })
    .await
  }

  /// How the run's requests were answered, with what auto mode did.
  fn decisions(&self) -> Vec<(DecidedBy, Option<Review>)> {
    self
      .db()
      .activity(self.id, 0)
      .expect("activity")
      .into_iter()
      .filter_map(|entry| match entry.event {
        AgentEvent::ToolDecided { decision, .. } => Some((decision.decided_by, decision.review)),
        _ => None,
      })
      .collect()
  }

  async fn deny(&mut self, request: PendingToolApproval) {
    self
      .scheduler
      .decide_tool_approval(self.id, request.id, ApprovalAnswer::Deny, Answerer::Person)
      .await
      .expect("denied");
  }

  /// What the fake reviewer wrote about where it ran: `key value` lines.
  fn logged(&self, key: &str) -> Vec<String> {
    std::fs::read_to_string(self.log.path().join("review.log"))
      .unwrap_or_default()
      .lines()
      .filter_map(|line| line.strip_prefix(&format!("{key} ")).map(str::to_owned))
      .collect()
  }

  /// Waits until the fake reviewer has written about where it runs.
  async fn reviewer_started(&self) -> String {
    tokio::time::timeout(PATIENCE, async {
      loop {
        if let Some(pid) = self.logged("pid").first() {
          return pid.clone();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
      }
    })
    .await
    .expect("the reviewer starts")
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_approves_a_command_that_serves_the_task_in_a_folder_of_its_own() {
  let mut started = Started::new(&claude("Run `cargo build`.")).await;
  assert_eq!(
    started.db().run_approval_mode(started.id).expect("read"),
    EffectiveMode::Manual(ManualBecause::Profile)
  );
  let run = started.ended().await;
  assert!(run.status().is_terminal());
  let decided = started.db().decided_tool_approvals(started.id).expect("decided");
  let millis = decided[0].decided_at.unix_millis() - decided[0].request.requested_at.unix_millis();
  // The review's own cost with the fake reviewer (start, the empty folder, the CLI, the verdict): about a
  // second on a loaded test machine.
  assert!(millis < 10_000, "review latency with the fake reviewer: {millis} ms");
  let decisions = started.decisions();
  assert_eq!(decisions.len(), 1, "{decisions:?}");
  assert_eq!(decisions[0].0, DecidedBy::Reviewer);
  assert!(
    matches!(&decisions[0].1, Some(Review::Approved { reason, .. }) if reason == "The fake reviewer finds it serves the task."),
    "{decisions:?}"
  );
  let counts = started.db().decision_counts(started.id).expect("counts");
  assert_eq!((counts.by_reviewer, counts.asked_by_reviewer, counts.by_you), (1, 0, 0));
  // It ran in an empty folder outside the project, removed after the review, with its system prompt.
  let cwd = PathBuf::from(started.logged("cwd").first().expect("logged"));
  assert!(!cwd.starts_with(started.project.repo.path()), "{}", cwd.display());
  assert_eq!(started.logged("empty"), ["true"]);
  assert_eq!(started.logged("instructions"), ["true"]);
  assert!(!cwd.exists(), "the review's folder is removed");
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_leaves_a_credential_read_to_the_person_without_a_review() {
  let mut started = Started::new(&claude("Run `cp ~/.orchester-test-token token.txt`.")).await;
  let (request, review) = started.left_to_person().await;
  assert_eq!(
    review,
    Review::Held {
      hold: ReviewHold::Secrets
    }
  );
  started.deny(request).await;
  started.ended().await;
  let decisions = started.decisions();
  assert_eq!(decisions, [(DecidedBy::PersonDeny, Some(review))]);
  let counts = started.db().decision_counts(started.id).expect("counts");
  assert_eq!((counts.by_reviewer, counts.asked_by_reviewer), (0, 1));
  assert!(started.logged("pid").is_empty(), "no reviewer ran");
}

/// ORC-339: a command Orchester cannot read to the end goes to the reviewer in a step that only reads too; the
/// reviewer is told the step's limits and decides.
#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_reviews_an_unknown_command_also_when_the_step_only_reads() {
  let mut on = Started::new(&claude("Run `./regen.sh`.")).await;
  on.ended().await;
  let decisions = on.decisions();
  assert_eq!(decisions.len(), 1, "{decisions:?}");
  assert_eq!(decisions[0].0, DecidedBy::Reviewer);
  assert_eq!(on.logged("pid").len(), 1, "the reviewer judged it");

  let mut read_only = Started::new(&asking_in("Run `./regen.sh`.", RepositoryAccess::ReadOnly)).await;
  read_only.ended().await;
  let decisions = read_only.decisions();
  assert_eq!(decisions.len(), 1, "{decisions:?}");
  assert_eq!(decisions[0].0, DecidedBy::Reviewer);
  assert_eq!(read_only.logged("pid").len(), 1, "the reviewer judged it");
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_reviews_a_plain_push_of_the_runs_branch_and_holds_a_force_push() {
  // The run records where origin pushes as it starts; the push must still go there.
  let project = project(&asking(
    "Run `git push origin orchester/orc-1-run-1`.",
    NetworkAccess::On,
  ));
  support::git(
    project.repo.path(),
    &["remote", "add", "origin", "https://example.invalid/kontur.git"],
  );
  let mut plain = Started::of(project, None).await;
  plain.ended().await;
  let decisions = plain.decisions();
  assert_eq!(decisions.len(), 1, "{decisions:?}");
  assert_eq!(decisions[0].0, DecidedBy::Reviewer);

  let mut forced = Started::new(&asking(
    "Run `git push --force origin orchester/orc-1-run-1`.",
    NetworkAccess::On,
  ))
  .await;
  let (request, review) = forced.left_to_person().await;
  assert_eq!(review, Review::Held { hold: ReviewHold::Push });
  forced.deny(request).await;
  forced.ended().await;
  assert!(forced.logged("pid").is_empty(), "no reviewer ran");
}

/// A run whose read-only step asks about `cargo test`, in a Cargo workspace whose repository ignores `ignored`.
async fn testing_in_a_read_only_step(ignored: &str) -> Started {
  let project = project(&asking_in(
    "Run `cargo test -p kontur --test smoke _`.",
    RepositoryAccess::ReadOnly,
  ));
  let root = project.repo.path();
  std::fs::write(
    root.join("Cargo.toml"),
    "[workspace]
members = []
",
  )
  .expect("manifest");
  std::fs::write(
    root.join(".gitignore"),
    format!(
      "{ignored}
"
    ),
  )
  .expect("ignore");
  support::git(root, &["add", "Cargo.toml", ".gitignore"]);
  support::git(root, &["commit", "--quiet", "-m", "A Cargo workspace"]);
  Started::of(project, None).await
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_reviews_a_build_in_a_read_only_step_that_writes_only_ignored_output() {
  let mut ignored = testing_in_a_read_only_step("/target").await;
  ignored.ended().await;
  let decisions = ignored.decisions();
  assert_eq!(decisions.len(), 1, "{decisions:?}");
  assert_eq!(decisions[0].0, DecidedBy::Reviewer);
  assert_eq!(ignored.logged("pid").len(), 1, "the reviewer judged it");

  let mut tracked = testing_in_a_read_only_step("/out").await;
  let (request, review) = tracked.left_to_person().await;
  assert_eq!(
    review,
    Review::Held {
      hold: ReviewHold::ReadOnlyStep
    }
  );
  tracked.deny(request).await;
  tracked.ended().await;
  assert!(tracked.logged("pid").is_empty(), "no reviewer ran");
}

/// A Codex Builder with any command in the worktree in a step with `repository` access, reviewed by its
/// own connection. (Network off is no confined step here: where Codex cannot enforce it, it stays on.)
async fn codex_escalating(repository: RepositoryAccess) -> Started {
  let setup = Setup {
    permissions: Some(AgentPermissions::new(
      repository,
      CommandPermission::AnyInWorktree,
      NetworkAccess::On,
    )),
    ..Setup::new("Escalate `cargo build`.")
  };
  let project = project(&setup);
  let codex = ProjectDb::open(project.repo.path())
    .expect("open")
    .connections()
    .expect("connections")
    .remove(0);
  let choice = ReviewerChoice {
    connection: codex.id(),
    model: ModelId::new("gpt-5.5-codex").expect("model"),
    effort: None,
  };
  Started::of(project, Some(choice)).await
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_leaves_every_codex_escalation_to_the_person() {
  for repository in [RepositoryAccess::Write, RepositoryAccess::ReadOnly] {
    let mut started = codex_escalating(repository).await;
    let (request, review) = started.left_to_person().await;
    assert!(request.command);
    assert_eq!(
      review,
      Review::Held {
        hold: ReviewHold::Escalation
      },
      "{repository:?}"
    );
    started.deny(request).await;
    started.ended().await;
    assert!(started.logged("pid").is_empty(), "no reviewer ran");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_asks_the_person_when_the_reviewer_asks_or_babbles() {
  let mut asked = Started::new(&claude("Run `cargo build`. Reviewer asks.")).await;
  let (request, review) = asked.left_to_person().await;
  assert!(
    matches!(&review, Review::Asked { reason, model } if reason == "The fake reviewer leaves it to the person."
      && model == "claude-haiku-4-5-20251001"),
    "{review:?}"
  );
  asked.deny(request).await;
  asked.ended().await;

  let mut babbled = Started::new(&claude("Run `cargo build`. Reviewer babbles.")).await;
  let (request, review) = babbled.left_to_person().await;
  assert!(
    matches!(
      review,
      Review::Failed {
        failure: ReviewFailure::Unreadable,
        ..
      }
    ),
    "{review:?}"
  );
  babbled.deny(request).await;
  babbled.ended().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_switched_to_manual_during_a_review_keeps_its_approval() {
  let mut started = Started::new(&claude("Run `cargo build`. Reviewer waits.")).await;
  started.under_review().await;
  started.reviewer_started().await;
  let switch = ManualSwitch {
    at: Timestamp::now(),
    by: Answerer::Person,
  };
  started
    .db()
    .switch_run_to_manual(started.id, switch)
    .expect("a person switches the run to Manual");
  // The run was in Manual already: the reviewer's approval, after three seconds, still answers the request.
  started.ended().await;
  let decisions = started.decisions();
  assert_eq!(decisions.len(), 1, "{decisions:?}");
  assert_eq!(decisions[0].0, DecidedBy::Reviewer);
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_stops_the_reviewer_when_the_person_answers_first() {
  let mut started = Started::new(&claude("Run `cargo build`. Reviewer hangs.")).await;
  let request = started.under_review().await;
  let pid = started.reviewer_started().await;
  assert!(is_running(&pid));
  started.deny(request).await;
  started.ended().await;
  let cwd = PathBuf::from(started.logged("cwd").first().expect("logged"));
  tokio::time::timeout(PATIENCE, async {
    while is_running(&pid) || cwd.exists() {
      tokio::time::sleep(Duration::from_millis(100)).await;
    }
  })
  .await
  .expect("the reviewer is stopped and its folder removed");
  assert_eq!(started.decisions(), [(DecidedBy::PersonDeny, None)]);
}

/// A run of a Claude Code Builder reviewed by a Codex connection of the project, with `model` and effort.
async fn reviewed_by_codex(brief: &'static str) -> Started {
  let setup = claude(brief);
  let project = project(&setup);
  let codex = support::agent_connection_of(AgentProvider::CodexCli);
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  db.create_connection(&codex).expect("a Codex connection");
  let choice = ReviewerChoice {
    connection: codex.id(),
    model: ModelId::new("gpt-5.5-codex").expect("model"),
    effort: Some("low".to_owned()),
  };
  Started::of(project, Some(choice)).await
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_reviews_with_a_codex_connection_in_its_read_only_sandbox() {
  let mut started = reviewed_by_codex("Run `cargo build`.").await;
  started.ended().await;
  let decisions = started.decisions();
  assert_eq!(decisions.len(), 1, "{decisions:?}");
  assert_eq!(decisions[0].0, DecidedBy::Reviewer);
  assert!(
    matches!(&decisions[0].1, Some(Review::Approved { reason, model })
      if reason == "The fake Codex reviewer finds it serves the task." && model == "gpt-5.5-codex"),
    "{decisions:?}"
  );
  let argv = started.logged("argv").remove(0);
  for part in [
    "-a never exec --json --ephemeral --ignore-user-config --ignore-rules --skip-git-repo-check --sandbox read-only",
    "-m gpt-5.5-codex -c model_reasoning_effort=low",
    "-c features.shell_tool=false -c tools.web_search=false --output-schema ",
  ] {
    assert!(argv.contains(part), "{part} in {argv}");
  }
  assert!(argv.ends_with(" -"), "the message comes on standard input: {argv}");
  let cwd = PathBuf::from(started.logged("cwd").first().expect("logged"));
  assert!(!cwd.starts_with(started.project.repo.path()), "{}", cwd.display());
  assert_eq!(started.logged("empty"), ["true"]);
  assert_eq!(started.logged("schema"), ["true"]);
  assert_eq!(started.logged("instructions"), ["true"]);
  assert!(!cwd.exists(), "the review's folder is removed");

  let mut failed = reviewed_by_codex("Run `cargo build`. Reviewer fails.").await;
  let (request, review) = failed.left_to_person().await;
  assert!(
    matches!(&review, Review::Failed { failure: ReviewFailure::CouldNotRun, model: Some(model), detail: Some(detail) }
      if model == "gpt-5.5-codex" && detail == "the reviewer could not run: the CLI exited with code 1: the fake model failed"),
    "{review:?}"
  );
  failed.deny(request).await;
  failed.ended().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_mode_keeps_a_request_from_the_person_until_the_reviewer_asks() {
  let mut started = Started::new(&claude("Run `cargo build`. Reviewer waits. Reviewer asks.")).await;
  let request = started.under_review().await;
  assert!(
    !started.waits_for_person(),
    "no wait for the person while the reviewer judges it"
  );
  let (asked, review) = started.left_to_person().await;
  assert_eq!(asked.id, request.id);
  assert!(matches!(review, Review::Asked { .. }), "{review:?}");
  assert!(started.db().reviewing_tool_approvals().expect("read").is_empty());
  tokio::time::timeout(PATIENCE, async {
    while !started.waits_for_person() {
      tokio::time::sleep(Duration::from_millis(50)).await;
    }
  })
  .await
  .expect("after the reviewer asks, the agent waits for the person");
  started.deny(asked).await;
  started.ended().await;

  // A hard rule asks the person at once, without a review.
  let mut held = Started::new(&claude("Run `cp ~/.orchester-test-token token.txt`.")).await;
  let (request, _) = held.left_to_person().await;
  assert!(held.db().reviewing_tool_approvals().expect("read").is_empty());
  assert!(held.waits_for_person());
  held.deny(request).await;
  held.ended().await;
}
