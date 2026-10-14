//! What All projects says about a project's runs without opening it: how many have an agent at
//! work, counted from the phase of each task's latest run, and what waits for the person, as every
//! Needs you counts it (`ProjectTasks::needs_you` in `orchester-workspace`).

use super::phase::TaskPhase;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunCounts {
  /// Runs with an agent at work.
  pub active: usize,
  /// What waits for the person, as every Needs you counts it.
  pub needs_you: usize,
}

impl RunCounts {
  /// Counts the runs at work among `phases`, one for each task with a readable latest run, beside
  /// `needs_you`, what waits for the person. Without a runner online no agent works, so a run
  /// recorded as running is interrupted rather than active ([`TaskPhase::without_runner`]).
  pub fn of(phases: impl IntoIterator<Item = TaskPhase>, runner_online: bool, needs_you: usize) -> Self {
    let active = phases
      .into_iter()
      .map(|phase| if runner_online { phase } else { phase.without_runner() })
      .filter(|phase| *phase == TaskPhase::Running)
      .count();
    Self { active, needs_you }
  }
}
