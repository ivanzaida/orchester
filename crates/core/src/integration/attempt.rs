//! A merge attempt into a run's base branch, as Review › Merges and the attempt page tell it
//! (`crates/desktop-app/design/orchester.pen`, board `Review · redesign`, 09 to 09l): who asked, from where, what came
//! of it, why it did not land, and how far it got through the merge's stages. Every attempt is recorded once, landed or
//! not; the record never changes.

use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

use super::{MergeKind, PreflightCause, PreflightStage};
use crate::{id::RunId, revision::Revision, time::Timestamp};

/// The most characters of a cause an attempt keeps.
pub const CAUSE_CHARS: usize = 1000;

/// Who asked for a merge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "who", rename_all = "snake_case")]
pub enum MergeAsker {
  Person,
  /// An MCP client or the Orchestrator, through `merge_run`'s policy.
  Automation,
  MergeQueue,
  /// Orchester trying a merge that did not land again by itself; `number` counts its tries of the run, from 1.
  Retry {
    number: NonZeroU32,
  },
}

/// Where a merge was asked from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeVia {
  /// *Merge into <base>* on the task page.
  TaskPage,
  /// The `merge_run` tool.
  MergeRun,
  MergeQueue,
}

/// A stage of a merge, in the order a merge passes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptStage {
  /// The policy an automation's merge goes through; only automation's attempts pass it.
  Policy,
  ProjectDatabase,
  IntegrationLock,
  ScratchFolder,
  CheckoutIdentity,
  GitMetadata,
  /// Whether the run may merge at all: not merged already, its task's latest run, its result not in Git already.
  ExistingResult,
  /// Git merging the run and bringing the result into the base branch.
  Merge,
  /// Recording the merge in the project database.
  Record,
}

impl AttemptStage {
  /// The stages in the order a merge passes them.
  const PASSED_IN_ORDER: [Self; 9] = [
    Self::Policy,
    Self::ProjectDatabase,
    Self::IntegrationLock,
    Self::ScratchFolder,
    Self::CheckoutIdentity,
    Self::GitMetadata,
    Self::ExistingResult,
    Self::Merge,
    Self::Record,
  ];

  /// The stages in the order an attempt lists them.
  const LISTED: [Self; 9] = [
    Self::Policy,
    Self::IntegrationLock,
    Self::ProjectDatabase,
    Self::ScratchFolder,
    Self::CheckoutIdentity,
    Self::GitMetadata,
    Self::ExistingResult,
    Self::Merge,
    Self::Record,
  ];

  /// The stage's name in logs and in an MCP client's answers.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Policy => "policy",
      Self::ProjectDatabase => "project_database",
      Self::IntegrationLock => "integration_lock",
      Self::ScratchFolder => "scratch_folder",
      Self::CheckoutIdentity => "checkout_identity",
      Self::GitMetadata => "git_metadata",
      Self::ExistingResult => "existing_result",
      Self::Merge => "merge",
      Self::Record => "record",
    }
  }

  fn order(self) -> usize {
    Self::PASSED_IN_ORDER
      .iter()
      .position(|stage| *stage == self)
      .expect("every stage is in the order")
  }
}

impl From<PreflightStage> for AttemptStage {
  fn from(stage: PreflightStage) -> Self {
    match stage {
      PreflightStage::IntegrationLock => Self::IntegrationLock,
      PreflightStage::ProjectDatabase => Self::ProjectDatabase,
      PreflightStage::ScratchFolder => Self::ScratchFolder,
      PreflightStage::CheckoutIdentity => Self::CheckoutIdentity,
      PreflightStage::GitMetadata => Self::GitMetadata,
      PreflightStage::ExistingResult => Self::ExistingResult,
      PreflightStage::Policy => Self::Policy,
    }
  }
}

/// How a stage went in an attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageResult {
  Passed,
  /// The attempt ended at this stage without passing it; its outcome says how.
  Refused,
  NotReached,
}

/// What came of an attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum AttemptOutcome {
  /// The run's work is in its base branch, at `revision`.
  Merged { revision: Revision, kind: MergeKind },
  /// `stage` refused; nothing was merged.
  Refused { stage: AttemptStage },
  /// Both sides changed these files; the merge was undone.
  Conflicts { files: Vec<String> },
  /// Git merged the run to `revision`, but the merge could not be recorded.
  NotRecorded { revision: Revision },
  /// Git did not answer at `stage`: it could not be run to the end, or what it printed made no sense.
  GitStopped { stage: AttemptStage },
  /// Git stopped while bringing the merge in, and these files it had written could not be put back.
  LeftChanged { files: Vec<String> },
  /// The app or the runner ended while the attempt was at `stage`.
  Interrupted { stage: AttemptStage },
  /// The merge queue stopped before the merge began, in its words.
  Stopped { words: String },
  /// The base branch or the run's branch moved since the run was checked; at `stage`, or before any (`None`).
  Moved { stage: Option<AttemptStage> },
}

impl AttemptOutcome {
  /// The outcome's name in the project database and in an MCP client's answers.
  pub const fn name(&self) -> &'static str {
    match self {
      Self::Merged { .. } => "merged",
      Self::Refused { .. } => "refused",
      Self::Conflicts { .. } => "conflicts",
      Self::NotRecorded { .. } => "not_recorded",
      Self::GitStopped { .. } => "git_stopped",
      Self::LeftChanged { .. } => "left_changed",
      Self::Interrupted { .. } => "interrupted",
      Self::Stopped { .. } => "stopped",
      Self::Moved { .. } => "moved",
    }
  }

  pub const fn landed(&self) -> bool {
    matches!(self, Self::Merged { .. })
  }

  /// The stage the attempt ended at without passing it; `None` once every stage passed, or none was reached.
  pub const fn stage(&self) -> Option<AttemptStage> {
    match self {
      Self::Merged { .. } | Self::Stopped { .. } => None,
      Self::Refused { stage } | Self::GitStopped { stage } | Self::Interrupted { stage } => Some(*stage),
      Self::Conflicts { .. } | Self::LeftChanged { .. } => Some(AttemptStage::Merge),
      Self::NotRecorded { .. } => Some(AttemptStage::Record),
      Self::Moved { stage } => *stage,
    }
  }
}

/// One merge attempt of a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeAttempt {
  run: RunId,
  at: Timestamp,
  asked_by: MergeAsker,
  via: MergeVia,
  outcome: AttemptOutcome,
  cause: Option<String>,
  stages: Vec<(AttemptStage, StageResult)>,
}

impl MergeAttempt {
  /// The attempt of `run` at `at`, which `asked_by` asked for `via`, with what came of it and why it did not land, at
  /// most [`CAUSE_CHARS`] of it. Its stages follow from its outcome: Policy only for automation's attempts.
  pub fn new(
    (run, at): (RunId, Timestamp),
    (asked_by, via): (MergeAsker, MergeVia),
    outcome: AttemptOutcome,
    cause: Option<&str>,
  ) -> Self {
    let stages = stages_of(asked_by, &outcome);
    Self {
      run,
      at,
      asked_by,
      via,
      outcome,
      cause: cause.map(bounded),
      stages,
    }
  }

  /// An attempt as it was recorded, with the stages it recorded.
  pub fn recorded(
    (run, at): (RunId, Timestamp),
    (asked_by, via): (MergeAsker, MergeVia),
    (outcome, cause): (AttemptOutcome, Option<String>),
    stages: Vec<(AttemptStage, StageResult)>,
  ) -> Self {
    Self {
      run,
      at,
      asked_by,
      via,
      outcome,
      cause,
      stages,
    }
  }

  pub const fn run(&self) -> RunId {
    self.run
  }

  pub const fn at(&self) -> Timestamp {
    self.at
  }

  pub const fn asked_by(&self) -> MergeAsker {
    self.asked_by
  }

  pub const fn via(&self) -> MergeVia {
    self.via
  }

  pub const fn outcome(&self) -> &AttemptOutcome {
    &self.outcome
  }

  /// Why it did not land, in the words of the OS, Git or Orchester; `None` when it landed or nothing more was said.
  pub fn cause(&self) -> Option<&str> {
    self.cause.as_deref()
  }

  /// Each stage the attempt lists, in order, with how it went.
  pub fn stages(&self) -> &[(AttemptStage, StageResult)] {
    &self.stages
  }
}

/// A recorded attempt, with its task and its place among its run's attempts (from 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedAttempt {
  pub task: crate::id::TaskId,
  pub sequence: u32,
  pub attempt: MergeAttempt,
}

/// An attempt that passed its preflight and has not ended: what records it interrupted when the app or the runner
/// ends before it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenAttempt {
  pub run: RunId,
  pub at: Timestamp,
  pub asked_by: MergeAsker,
  pub via: MergeVia,
  pub stage: AttemptStage,
}

impl OpenAttempt {
  /// This attempt, interrupted at its stage, its bring-in having left `left_changed` changed in the checkout.
  pub fn interrupted(&self, left_changed: &[String]) -> MergeAttempt {
    let ended = "the app or the runner ended during the merge";
    let cause = if left_changed.is_empty() {
      ended.to_owned()
    } else {
      format!(
        "{ended} while bringing it in, and left changed: {}",
        left_changed.join(", ")
      )
    };
    MergeAttempt::new(
      (self.run, self.at),
      (self.asked_by, self.via),
      AttemptOutcome::Interrupted { stage: self.stage },
      Some(&cause),
    )
  }
}

/// The cause of a preflight's refusal, as an attempt keeps it.
pub fn cause_text(cause: &PreflightCause) -> String {
  bounded(&cause.to_string())
}

/// `text`'s first [`CAUSE_CHARS`] characters.
fn bounded(text: &str) -> String {
  text.chars().take(CAUSE_CHARS).collect()
}

/// How each stage went for an attempt of `asked_by` that ended as `outcome`, as listed: the stages before the one it
/// ended at passed, that one refused, the others were not reached.
fn stages_of(asked_by: MergeAsker, outcome: &AttemptOutcome) -> Vec<(AttemptStage, StageResult)> {
  let ended = outcome.stage().map(AttemptStage::order);
  let none_reached = matches!(
    outcome,
    AttemptOutcome::Stopped { .. } | AttemptOutcome::Moved { stage: None }
  );
  AttemptStage::LISTED
    .into_iter()
    .filter(|stage| *stage != AttemptStage::Policy || asked_by == MergeAsker::Automation)
    .map(|stage| {
      let result = match ended {
        _ if none_reached => StageResult::NotReached,
        None => StageResult::Passed,
        Some(ended) if stage.order() < ended => StageResult::Passed,
        Some(ended) if stage.order() == ended => StageResult::Refused,
        Some(_) => StageResult::NotReached,
      };
      (stage, result)
    })
    .collect()
}
