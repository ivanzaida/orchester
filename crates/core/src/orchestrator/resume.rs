//! What the Orchestrator chat's `resume_run` may do with a run (`docs/design/orchestrator-toolset.md`, decision 5):
//! resume one a pause stopped; continue one a crash, the runner going away or *Quit now* interrupted, from its last
//! checkpoint, only while the person lets it (*Let the Orchestrator continue interrupted runs*); and nothing else
//! that stopped: a limit, a person's cancel, the runner failing to record it or its worktree gone leave the run to
//! the person whatever the switches say. A turn an interrupted run's end started by itself continues the run once,
//! unless the run was interrupted again soon after the time before ([`Continuing::after`]).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{
  run::{InterruptCause, PauseReason, RunLimit, RunStatus},
  time::Timestamp,
};

/// How soon after an interruption another one of the same run leaves it to the person: a run that keeps breaking is
/// not continued in a loop.
pub const AGAIN_WITHIN: Duration = Duration::from_secs(10 * 60);

/// What the chat's `resume_run` does with a run that stands so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatResume {
  /// A pause stopped it: the chat resumes it.
  Paused,
  /// A crash, the runner going away or *Quit now* interrupted it: the chat continues it from its last checkpoint,
  /// while the person lets it.
  Interrupted(InterruptCause),
  /// It is the person's to resume or continue, whatever the switches say.
  PersonOnly(PersonsStop),
  /// It is not stopped so: the tool tells what resuming it does.
  Other,
}

/// Why a stopped run is the person's to take on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonsStop {
  /// It stopped at this limit: raising it, or resuming after the daily budget, is the person's.
  Limit(RunLimit),
  /// The runner could not record or carry it on: a person looks at why before it goes on.
  RunnerFailed,
  /// A person canceled it.
  Canceled,
  /// It was interrupted and its worktree is gone: it never continues.
  WorktreeGone,
}

impl ChatResume {
  /// What the chat's `resume_run` does with a run of `status`, its worktree there when `worktree_present`.
  pub const fn of(status: &RunStatus, worktree_present: bool) -> Self {
    match status {
      RunStatus::Paused(PauseReason::Requested) => Self::Paused,
      RunStatus::Paused(PauseReason::LimitReached(limit)) => Self::PersonOnly(PersonsStop::Limit(*limit)),
      RunStatus::Interrupted(_) if !worktree_present => Self::PersonOnly(PersonsStop::WorktreeGone),
      RunStatus::Interrupted(InterruptCause::RunnerFailed) => Self::PersonOnly(PersonsStop::RunnerFailed),
      RunStatus::Interrupted(cause) => Self::Interrupted(*cause),
      RunStatus::Cancelled => Self::PersonOnly(PersonsStop::Canceled),
      RunStatus::Pending
      | RunStatus::Running
      | RunStatus::WaitingForPerson(_)
      | RunStatus::Succeeded
      | RunStatus::Failed(_) => Self::Other,
    }
  }
}

/// What the Orchestrator turn an interrupted run's end starts by itself does about the run, while the person lets the
/// Orchestrator both answer agents and continue interrupted runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Continuing {
  /// It continues the run itself, once, from its last checkpoint.
  Continues,
  /// The run was interrupted again within [`AGAIN_WITHIN`] of the time before: it is left to the person.
  InterruptedAgain,
}

impl Continuing {
  /// For a run interrupted at `interruptions`, oldest first, the last one now.
  pub fn after(interruptions: &[Timestamp]) -> Self {
    let window = i64::try_from(AGAIN_WITHIN.as_millis()).expect("ten minutes of milliseconds fit in i64");
    match interruptions {
      [.., before, last] if last.unix_millis() - before.unix_millis() <= window => Self::InterruptedAgain,
      _ => Self::Continues,
    }
  }
}
