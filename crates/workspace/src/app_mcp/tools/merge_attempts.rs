//! `get_run`'s `merge_attempts`: every merge attempt of the run, newest first (`orchester_core::integration::attempt`).
//! What the OS, Git or the merge queue said, and the files an attempt names, reach an MCP client only through
//! `merge_run`'s policy ([`LegacyPolicy`]), as `merge_run`'s own refusals do; a policy that cannot be made passes none
//! of it through.

use std::path::Path;

use orchester_core::{
  id::RunId,
  integration::{
    MergeKind,
    attempt::{AttemptOutcome, MergeAsker, MergeAttempt, MergeVia, RecordedAttempt, StageResult},
  },
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::{Translate, wire};
use crate::{app_mcp::error::ToolError, messages::Message, secrets::Vault, tasks::local_integration::LegacyPolicy};

/// A run's attempts as `get_run` gives them, but for their causes, which are told in the app's words
/// ([`Attempts::told`]).
pub(super) struct Attempts {
  read: Vec<Value>,
  causes: Vec<Option<Message>>,
}

impl Attempts {
  /// `run`'s attempts in `db`, the project at `root`'s, each cause and file list as the policy made with `vault` lets
  /// an MCP client read it.
  pub(super) fn of_run(db: &ProjectDb, (root, run): (&Path, RunId), vault: &Vault) -> Result<Self, ToolError> {
    let attempts = db.merge_attempts(run)?;
    let policy = if attempts.iter().any(|recorded| says_more(&recorded.attempt)) {
      LegacyPolicy::new(root, run, vault.clone())
        .inspect_err(|error| tracing::warn!(%error, %run, "the merge attempts' causes are not told: no policy"))
        .ok()
    } else {
      None
    };
    let (read, causes) = attempts
      .iter()
      .map(|recorded| attempt_json(recorded, policy.as_ref()))
      .unzip();
    Ok(Self { read, causes })
  }

  /// The attempts, each cause in the app's words (`translate`).
  pub(super) fn told(self, translate: &Translate) -> Value {
    let told = self.read.into_iter().zip(self.causes).map(|(mut read, cause)| {
      read["cause"] = json!(cause.map(|cause| translate.text(&cause)));
      read
    });
    Value::Array(told.collect())
  }
}

/// Whether `attempt` says more than its outcome: a cause, the queue's words or files.
fn says_more(attempt: &MergeAttempt) -> bool {
  attempt.cause().is_some()
    || matches!(
      attempt.outcome(),
      AttemptOutcome::Stopped { .. } | AttemptOutcome::Conflicts { .. } | AttemptOutcome::LeftChanged { .. }
    )
}

/// `recorded` as `get_run` gives it, but its cause, which goes apart as `policy` lets it through.
fn attempt_json(recorded: &RecordedAttempt, policy: Option<&LegacyPolicy>) -> (Value, Option<Message>) {
  let attempt = &recorded.attempt;
  let (who, retry) = match attempt.asked_by() {
    MergeAsker::Person => ("person", None),
    MergeAsker::Automation => ("automation", None),
    MergeAsker::MergeQueue => ("merge_queue", None),
    MergeAsker::Retry { number } => ("retry", Some(number.get())),
  };
  let via = match attempt.via() {
    MergeVia::TaskPage => "task_page",
    MergeVia::MergeRun => "merge_run",
    MergeVia::MergeQueue => "merge_queue",
  };
  let mut read = json!({
    "sequence": recorded.sequence,
    "at": wire::time(attempt.at()),
    "who": who,
    "retry": retry,
    "via": via,
    "outcome": attempt.outcome().name(),
    "stage": attempt.outcome().stage().map(|stage| stage.name()),
    "stages": attempt.stages().iter().map(|(stage, result)| json!({
      "stage": stage.name(),
      "result": match result {
        StageResult::Passed => "passed",
        StageResult::Refused => "refused",
        StageResult::NotReached => "not_reached",
      },
    })).collect::<Vec<_>>(),
  });
  let files = match attempt.outcome() {
    AttemptOutcome::Merged { revision, kind } => {
      read["revision"] = json!(revision.to_string());
      read["kind"] = json!(kind_name(*kind));
      None
    }
    AttemptOutcome::NotRecorded { revision } => {
      read["revision"] = json!(revision.to_string());
      None
    }
    AttemptOutcome::Conflicts { files } | AttemptOutcome::LeftChanged { files } => Some(files.clone()),
    _ => None,
  };
  if let Some(files) = files {
    read["files"] = json!(policy.map_or_else(Vec::new, |policy| policy.safe_paths(files)));
  }
  let said = match attempt.outcome() {
    AttemptOutcome::Stopped { words } => Some(words.as_str()),
    _ => attempt.cause(),
  };
  let cause = said.and_then(|said| {
    let cause = Message::new("appMcp.mergeAttemptCause").with("cause", said.to_owned());
    policy.map(|policy| policy.safe_message(cause))
  });
  (read, cause)
}

const fn kind_name(kind: MergeKind) -> &'static str {
  match kind {
    MergeKind::FastForward => "fast_forward",
    MergeKind::MergeCommit => "merge_commit",
    MergeKind::AlreadyMerged => "already_merged",
    MergeKind::Squash => "squash",
  }
}
