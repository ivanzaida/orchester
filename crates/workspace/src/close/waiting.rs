//! Tasks that only wait for a person while the window closes: no agent works on them, so closing
//! does not change them, and the closing panel names them once under its rows (`Note` `QeO8y` in
//! `u3Fw36`). Their agent's session resumes with the answer or the decision.

use std::sync::Arc;

use orchester_core::run::TaskPhase;

use crate::tasks::{
  data::ProjectTasks,
  list::{Latest, LatestRun},
};

/// What a task waits for from a person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersonWait {
  /// A decision on its plan.
  Plan,
  /// An answer to its agent's question.
  Answer,
  /// A decision on how its run goes on after a step's retries ran out or it broke again.
  Decision,
}

/// A task that waits only for a person, by its key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitingTask {
  pub key: Arc<str>,
  pub wait: PersonWait,
}

/// The tasks of `tasks` whose latest run waits only for a person, in the order of the list. A run
/// that waits on a tool request is not one: its agent holds the request, so it is at work.
pub fn waiting_for_person(tasks: &ProjectTasks) -> Vec<WaitingTask> {
  tasks
    .tasks
    .iter()
    .filter_map(|task| {
      let latest = tasks.phases.get(&task.id()).and_then(Latest::run)?;
      let wait = person_wait(latest)?;
      Some(WaitingTask {
        key: task.key().to_string().into(),
        wait,
      })
    })
    .collect()
}

fn person_wait(latest: &LatestRun) -> Option<PersonWait> {
  match latest.phase {
    TaskPhase::Waiting if latest.plan.is_some() => Some(PersonWait::Plan),
    TaskPhase::Waiting if latest.question.is_some() => Some(PersonWait::Answer),
    TaskPhase::Stuck => Some(PersonWait::Decision),
    _ => None,
  }
}
