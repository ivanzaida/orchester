//! Execution of a task: a run pins its configuration in a [`RunSnapshot`], executes the
//! pipeline in attempts of step executions, records checkpoints and spending, and changes only
//! through typed transitions. Every accepted transition yields the [`RunEvent`]s to append;
//! the event log is the source of truth and the current state is derivable from it
//! ([`Run::replay`]). See `docs/architecture/storage.md` (Execution, State machines).

mod apply;
mod attempt;
mod branch;
mod error;
mod event;
mod number;
mod snapshot;
mod state;
mod status;
mod step;
mod transition;

pub use self::{
  attempt::{Attempt, Checkpoint, StepExecution},
  branch::{BranchName, BranchNameError},
  error::RunError,
  event::{EventSequence, PlannedExecution, RunCreated, RunEvent, RunEventKind},
  number::RunNumber,
  snapshot::{PinnedVersion, PublishedVersions, RunSnapshot, SnapshotError, StepExecutor, StepSnapshot},
  state::{NewRun, Run, StoredRun},
  status::{
    AttemptReason, AttemptStatus, ContinueTrigger, InterruptCause, PlanDecision, RunFailure, RunStatus, StepStatus,
    WaitReason,
  },
};
