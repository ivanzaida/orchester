//! When the code viewer reads a version again, so its tree and its file follow changes made outside it (the
//! person's decision of 2026-10-15): a run's worktree on each change the runner reports of the project's runs
//! and transcripts while an agent works in it; the checkout, the base branch and a run's worktree when the
//! project's merge and integration operations change, which write them or remove the worktree; a commit
//! never, since it cannot change. Nothing polls: the reads follow the app's own change signals.

use orchester_core::{id::RunId, run::TaskPhase};

use super::Version;
use crate::tasks::{data::ProjectTasks, list::Latest};

/// What wakes the reads of a version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Follow {
  /// The runner's changes of runs and transcripts, and the integration operations.
  RunChanges,
  /// The integration operations only.
  Integration,
  Never,
}

impl Follow {
  /// How the page shows `version` while the project's tasks are `tasks` (`None` before they are read).
  pub fn of(version: &Version, tasks: Option<&ProjectTasks>) -> Self {
    match version {
      Version::Run(run) if tasks.is_some_and(|tasks| agent_at_work(tasks, *run)) => Self::RunChanges,
      Version::Run(_) | Version::Checkout | Version::Base => Self::Integration,
      Version::Revision(_) => Self::Never,
    }
  }
}

/// Whether `run` is the running latest run of one of the tasks: an agent works in its worktree.
pub fn agent_at_work(tasks: &ProjectTasks, run: RunId) -> bool {
  tasks
    .phases
    .values()
    .filter_map(Latest::run)
    .any(|latest| latest.id == run && latest.phase == TaskPhase::Running)
}
