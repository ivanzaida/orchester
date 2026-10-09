//! Returns and restarts (02.B7 `zyusO`): an attempt that a failure's route returned to an earlier
//! step, that restarted a step which broke, or that a person returned with a note, starts with a
//! mark of its own before the header of the step it starts from. It is never part of a tool group.

use std::sync::Arc;

use orchester_core::{
  resource::StepKey,
  run::{Attempt, AttemptReason},
  time::Timestamp,
};

use crate::tasks::detail::RunView;

/// Why an attempt started, as its mark reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkKind {
  /// A failure's route returned the work to `to`; `retry` is which retry of how many the route
  /// allows, for an automatic one.
  Returned {
    to: Arc<str>,
    agent: Arc<str>,
    failed: Arc<str>,
    failed_attempt: u32,
    retry: (usize, u32),
    /// The other steps of the attempt, which run again.
    after: Vec<Arc<str>>,
  },
  /// `step` broke and starts again as a fresh session.
  Restarted { step: Arc<str> },
  /// A person returned the work to `to` with `note`.
  ReturnedWithNote { to: Arc<str>, note: Arc<str> },
}

/// A return or restart before the header of the step its attempt starts from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
  pub attempt: u32,
  pub at: Option<Timestamp>,
  pub by_person: bool,
  pub kind: MarkKind,
}

/// The mark of the attempt at `index` of `run`'s attempts, when it is a return or a restart.
pub fn mark_of(run: &RunView, index: usize) -> Option<Mark> {
  let attempts = run.run.attempts();
  let attempt = attempts.get(index)?;
  let start = run.attempt_starts.iter().find(|start| start.attempt == attempt.id());
  let name = |step: &StepKey| -> Arc<str> {
    run
      .steps
      .iter()
      .find(|view| view.key == *step)
      .map_or_else(|| step.as_str().into(), |view| view.name.clone())
  };
  let kind = match attempt.reason() {
    AttemptReason::Retry { failed_step, return_to } => MarkKind::Returned {
      to: name(return_to),
      agent: run
        .steps
        .iter()
        .find(|view| view.key == *return_to)
        .and_then(|view| view.agent.clone())
        .unwrap_or_else(|| name(return_to)),
      failed: name(failed_step),
      failed_attempt: index
        .checked_sub(1)
        .map_or(0, |earlier| attempts[earlier].number().get()),
      retry: retry_of(run, &attempts[..=index], failed_step),
      after: others(attempt, return_to).map(name).collect(),
    },
    AttemptReason::Restart { step } => MarkKind::Restarted { step: name(step) },
    AttemptReason::Returned { return_to, note, .. } => MarkKind::ReturnedWithNote {
      to: name(return_to),
      note: note.as_str().into(),
    },
    AttemptReason::Initial | AttemptReason::FollowUp { .. } | AttemptReason::Recovery { .. } => return None,
  };
  Some(Mark {
    attempt: attempt.number().get(),
    at: start.map(|start| start.at),
    by_person: start.is_some_and(|start| start.by_person),
    kind,
  })
}

/// Which retry of `failed_step`'s route `attempts` ends with, and how many the route allows.
fn retry_of(run: &RunView, attempts: &[Attempt], failed_step: &StepKey) -> (usize, u32) {
  let taken = attempts
    .iter()
    .filter(|attempt| matches!(attempt.reason(), AttemptReason::Retry { failed_step: step, .. } if step == failed_step))
    .count();
  let allowed = run
    .run
    .snapshot()
    .body()
    .step(failed_step)
    .and_then(|step| step.on_failure.as_ref())
    .map_or(0, |route| route.max_retries.get());
  (taken, allowed)
}

fn others<'a>(attempt: &'a Attempt, start: &'a StepKey) -> impl Iterator<Item = &'a StepKey> {
  attempt
    .executions()
    .iter()
    .map(|execution| execution.step())
    .filter(move |step| *step != start)
}
