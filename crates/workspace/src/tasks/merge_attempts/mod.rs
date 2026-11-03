//! Recording every merge attempt into a run's base branch (`orchester_core::integration::attempt`): from the task page,
//! `merge_run` and the merge queue, landed or not, with how far it got. A record that cannot be written is logged and
//! changes nothing of the merge: a merge that landed stands. An attempt that passed its preflight is kept open while
//! it goes on, and the next merge, holding the project's integration lease so that no other one goes on, records one
//! left open as interrupted.

pub mod read;

use std::{path::Path, thread};

use orchester_core::{
  command::CommandError,
  id::RunId,
  integration::{
    GitError, MergeError,
    attempt::{AttemptOutcome, AttemptStage, MergeAsker, MergeAttempt, MergeVia, OpenAttempt, cause_text},
  },
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::integration::{IntegrationProblem, Merged};

/// Who asked for a merge, and from where.
pub type Asked = (MergeAsker, MergeVia);

/// A merge attempt as it goes on: the stage it is at, until it ends ([`Attempt::end`]). One that ends by a panic is
/// recorded interrupted at its stage.
pub(crate) struct Attempt<'a> {
  root: &'a Path,
  run: RunId,
  asked: Asked,
  at: Timestamp,
  stage: AttemptStage,
  open: bool,
  ended: bool,
}

impl<'a> Attempt<'a> {
  /// The attempt of `run` in the project at `root` that `asked` asked for, at the first stage of a merge in it.
  pub(crate) fn begin(root: &'a Path, run: RunId, asked: Asked) -> Self {
    Self {
      root,
      run,
      asked,
      at: Timestamp::now(),
      stage: AttemptStage::ProjectDatabase,
      open: false,
      ended: false,
    }
  }

  /// The attempt reached `stage` past its preflight, and is kept open in `db` at it. The first time, the attempts left
  /// open are recorded interrupted: the caller holds the integration lease, so none of them goes on.
  pub(crate) fn reach(&mut self, db: &mut ProjectDb, stage: AttemptStage) {
    self.stage = stage;
    if !self.open {
      self.open = true;
      if let Err(error) = settle_interrupted(db) {
        tracing::warn!(%error, "cannot record the merge attempts the app or the runner left");
      }
    }
    let open = OpenAttempt {
      run: self.run,
      at: self.at,
      asked_by: self.asked.0,
      via: self.asked.1,
      stage,
    };
    if let Err(error) = db.open_merge_attempt(&open) {
      tracing::warn!(%error, run = %self.run, stage = stage.name(), "cannot keep the merge attempt open");
    }
  }

  /// Records how the attempt ended.
  pub(crate) fn end(mut self, merged: &Result<Merged, IntegrationProblem>) {
    self.ended = true;
    let (outcome, cause) = match merged {
      Ok(merged) => (
        AttemptOutcome::Merged {
          revision: merged.merge.revision.clone(),
          kind: merged.merge.kind,
        },
        None,
      ),
      Err(problem) => outcome_of(problem, self.stage),
    };
    let attempt = MergeAttempt::new((self.run, self.at), self.asked, outcome, cause.as_deref());
    record(self.root, &attempt);
  }
}

impl Drop for Attempt<'_> {
  fn drop(&mut self) {
    if self.ended || !thread::panicking() {
      return;
    }
    let attempt = MergeAttempt::new(
      (self.run, self.at),
      self.asked,
      AttemptOutcome::Interrupted { stage: self.stage },
      Some("the merge ended unexpectedly"),
    );
    record(self.root, &attempt);
  }
}

/// Records that the attempt of `run` that `asked` asked for did not reach a merge: `problem` stopped it at `stage`.
pub(crate) fn record_refused(
  root: &Path,
  run: RunId,
  asked: Asked,
  (stage, problem): (AttemptStage, &IntegrationProblem),
) {
  let (outcome, cause) = outcome_of(problem, stage);
  record(
    root,
    &MergeAttempt::new((run, Timestamp::now()), asked, outcome, cause.as_deref()),
  );
}

/// Records `attempt` in the project at `root`, unless the merge queue tries again what its last attempt of the run
/// found already: a delivery it polls while the same thing holds it back is the same attempt still waiting, and only
/// its open attempt ends. A record that cannot be written is logged.
pub(crate) fn record(root: &Path, attempt: &MergeAttempt) {
  let recorded = ProjectDb::open(root).and_then(|mut db| {
    if attempt.asked_by() == MergeAsker::MergeQueue && repeats_last(&db, attempt)? {
      return db.close_merge_attempt(attempt.run());
    }
    db.record_merge_attempt(attempt).map(drop)
  });
  if let Err(error) = recorded {
    tracing::warn!(%error, run = %attempt.run(), outcome = attempt.outcome().name(), "cannot record the merge attempt; the merge's own outcome stands");
  }
}

fn repeats_last(db: &ProjectDb, attempt: &MergeAttempt) -> Result<bool, StoreError> {
  Ok(db.merge_attempts(attempt.run())?.first().is_some_and(|last| {
    let last = &last.attempt;
    (last.asked_by(), last.via(), last.outcome(), last.cause())
      == (attempt.asked_by(), attempt.via(), attempt.outcome(), attempt.cause())
  }))
}

/// Records each attempt left open in `db` as interrupted.
fn settle_interrupted(db: &mut ProjectDb) -> Result<(), StoreError> {
  for open in db.open_merge_attempts()? {
    tracing::warn!(run = %open.run, stage = open.stage.name(), "a merge attempt ended with the app or the runner");
    db.record_merge_attempt(&open.interrupted())?;
  }
  Ok(())
}

/// What `problem`, which ended an attempt at `stage` unless it names its own, makes of it, with its cause.
fn outcome_of(problem: &IntegrationProblem, stage: AttemptStage) -> (AttemptOutcome, Option<String>) {
  let refused = |cause: String| (AttemptOutcome::Refused { stage }, Some(cause));
  match problem {
    IntegrationProblem::Preflight { stage, cause } => (
      AttemptOutcome::Refused { stage: (*stage).into() },
      Some(cause_text(cause)),
    ),
    IntegrationProblem::Merge(error) => merge_outcome(error, stage),
    IntegrationProblem::NotRecorded { merge, problem } => (
      AttemptOutcome::NotRecorded {
        revision: merge.revision.clone(),
      },
      Some(problem.key.to_owned()),
    ),
    IntegrationProblem::HeldLocally(_) => refused("a local integration holds the checkout".to_owned()),
    IntegrationProblem::AlreadyMerged => refused("the run was merged already".to_owned()),
    IntegrationProblem::Store(message) => refused(message.key.to_owned()),
    IntegrationProblem::Read(error) => refused(error.to_string()),
    IntegrationProblem::Cleanup(error) => refused(error.to_string()),
    other => refused(format!("{other:?}")),
  }
}

fn merge_outcome(error: &MergeError, stage: AttemptStage) -> (AttemptOutcome, Option<String>) {
  let outcome = match error {
    MergeError::Conflict { files } => return (AttemptOutcome::Conflicts { files: files.clone() }, None),
    MergeError::LeftChanged { files, detail } => {
      return (
        AttemptOutcome::LeftChanged { files: files.clone() },
        Some(detail.clone()),
      );
    }
    // Git that is not there, or that could not be given its arguments, never ran: the merge was refused.
    MergeError::Git(GitError::Run(CommandError::NotFound | CommandError::UnsafeScriptArgument { .. })) => {
      AttemptOutcome::Refused { stage }
    }
    MergeError::Git(GitError::Run(_) | GitError::Unrecognized { .. }) => AttemptOutcome::GitStopped { stage },
    MergeError::BaseMoved { .. } | MergeError::BranchMoved { .. } => AttemptOutcome::Moved { stage: Some(stage) },
    _ => AttemptOutcome::Refused { stage },
  };
  (outcome, Some(error.to_string()))
}
