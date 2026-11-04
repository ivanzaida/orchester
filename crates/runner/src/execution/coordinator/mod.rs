//! The coordinator's side of execution, one per project beside the project database
//! (`docs/architecture/remote-runners.md`, "The split"): creating runs, the queue, task groups and blocked runs,
//! the gates that hold work back (agent slots, the daily budget, plan limits, low disk space), the Orchestrator
//! chat, task estimates, the scan for queued messages, the checks at startup, and the scheduler's side of the
//! requests for a run (pause, plans, permissions, questions, tool requests, stop turn, recovery and continuing a
//! run that is not executing). It hands each run it starts to the executor (`super::executor`), and requests for
//! a run being executed to its task (`super::executor::control`).

pub(super) mod again;
pub(super) mod attach;
pub(super) mod blocked_runs;
pub(super) mod board;
pub(super) mod browser;
#[cfg(feature = "test-agent")]
pub(super) mod channel;
pub(super) mod chat;
pub(super) mod conversation;
pub(super) mod estimate;
pub(super) mod idle;
pub(super) mod link;
pub(super) mod low_disk;
pub(super) mod missing_checkpoints;
pub(super) mod permissions;
pub(super) mod plan;
pub(super) mod plan_limit;
pub(super) mod prepare;
pub(super) mod question;
pub(super) mod recovery;
pub(super) mod returned;
pub(super) mod run_folder;
pub(super) mod sink;
pub(super) mod stuck;
pub(super) mod task_groups;
pub(super) mod tool_approval;
pub(super) mod turn_stop;

use super::{Notifier, Scheduler, failed, read_failed};
