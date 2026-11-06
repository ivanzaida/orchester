//! A person's stop of a subscription (× or /unwatch) ends it at once and cancels and pauses nothing: the runs of its
//! tasks go on as they are. The stop line lists what was left open, read here from the phases of those tasks.

use serde::{Deserialize, Serialize};

use crate::{id::TaskId, run::TaskPhase};

/// Work of a task a stop leaves open, as the stop line names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenWork {
  /// Its run waits for a person: a step waits, or the run does.
  Waits,
  Runs,
  Queued,
  StoppedAtLimit,
}

impl OpenWork {
  /// The open work of a task in `phase`; `None` when the stop line has nothing to say of it.
  pub const fn of(phase: TaskPhase) -> Option<Self> {
    match phase {
      TaskPhase::Waiting | TaskPhase::Stuck => Some(Self::Waits),
      TaskPhase::Running => Some(Self::Runs),
      TaskPhase::Queued => Some(Self::Queued),
      TaskPhase::StoppedAtLimit => Some(Self::StoppedAtLimit),
      TaskPhase::Draft
      | TaskPhase::Paused
      | TaskPhase::Interrupted
      | TaskPhase::Succeeded
      | TaskPhase::Merged
      | TaskPhase::Failed
      | TaskPhase::Cancelled => None,
    }
  }
}

/// What a person's stop left open, in the order of the tasks given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LeftOpen {
  tasks: Vec<(TaskId, OpenWork)>,
}

impl LeftOpen {
  /// What stopping a subscription whose tasks are in these phases leaves open. It only reads them: no run changes.
  pub fn after_stop(tasks: impl IntoIterator<Item = (TaskId, TaskPhase)>) -> Self {
    Self {
      tasks: tasks
        .into_iter()
        .filter_map(|(task, phase)| OpenWork::of(phase).map(|work| (task, work)))
        .collect(),
    }
  }

  pub fn tasks(&self) -> &[(TaskId, OpenWork)] {
    &self.tasks
  }

  /// How many tasks were left with `work`.
  pub fn count(&self, work: OpenWork) -> usize {
    self.tasks.iter().filter(|(_, left)| *left == work).count()
  }

  pub fn is_empty(&self) -> bool {
    self.tasks.is_empty()
  }
}
