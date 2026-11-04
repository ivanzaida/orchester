//! The executor's side of execution, one per run where the run's worktree is
//! (`docs/architecture/remote-runners.md`, "The split"): the run's worktrees, made from the coordinator's start order
//! and removed for Run again, and the tips of the runs' branches; the run's task and its steps, agent processes and
//! the CLI relay, tool steps, checkpoints, handoffs, the run's side of plans, questions, permissions and tool
//! requests, how it halts and what holds its steps back (its workspace, the disk guard), the code index of the run's
//! base, its environment, the warm workspaces it borrows and the merge queue's work in its worktree. It reaches the
//! project through the coordinator link ([`link`]), the store and the scheduler handle (`super::Scheduler`), not
//! through `super::coordinator`:
//! `crates/runner/tests/execution_split.rs` fails on any new place that names it, and lists the places that still do.

pub(super) mod attached;
pub(super) mod broken;
pub(super) mod checkpoint;
pub(super) mod code_index_rebuild;
pub(super) mod codebase_index;
pub(super) mod connection_check;
pub(super) mod control;
pub(super) mod conversation;
pub(super) mod disk_guard;
pub(super) mod environment;
pub(super) mod folder_state;
pub(super) mod folders;
pub(super) mod halt;
pub(super) mod handoff;
pub(super) mod held;
pub mod link;
pub(super) mod merge_queue;
pub(super) mod permissions;
pub(super) mod plan;
pub(super) mod pool;
pub(super) mod prepared;
pub(super) mod question;
pub(super) mod record;
pub(super) mod removal;
pub(super) mod run;
pub(super) mod run_branches;
pub(super) mod run_git;
pub(super) mod sink;
pub(super) mod start;
pub(super) mod step;
pub(super) mod tool;
pub(super) mod tool_approval;
pub(super) mod turn_stop;
pub(super) mod work;
pub(super) mod workspace;
pub(super) mod worktree_changes;
pub(super) mod worktree_removal;
pub(super) mod worktree_sweep;

use super::{Notifier, failed};
