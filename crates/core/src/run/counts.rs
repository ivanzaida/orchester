//! What All projects says about a project's runs without opening it: how many have an agent at
//! work and how many wait for a person, counted from the phase of each task's latest run.

use super::phase::TaskPhase;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunCounts {
  /// Runs with an agent at work.
  pub active: usize,
  /// Runs waiting for a person: a step's plan decision, question or approval, or the run's own
  /// decision after its retries ran out.
  pub needs_you: usize,
}

impl RunCounts {
  /// Counts `phases`, one for each task with a readable latest run. Without a runner online no
  /// agent works, so a run recorded as running is interrupted rather than active
  /// ([`TaskPhase::without_runner`]); a step waiting for a person still waits.
  pub fn of(phases: impl IntoIterator<Item = TaskPhase>, runner_online: bool) -> Self {
    phases
      .into_iter()
      .map(|phase| if runner_online { phase } else { phase.without_runner() })
      .fold(Self::default(), |counts, phase| match phase {
        TaskPhase::Running => Self {
          active: counts.active + 1,
          ..counts
        },
        phase if phase.needs_you() => Self {
          needs_you: counts.needs_you + 1,
          ..counts
        },
        _ => counts,
      })
  }
}
