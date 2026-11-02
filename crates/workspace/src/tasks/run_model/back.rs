//! The next step starts on its own profile (ORC-322, the last event line of `08f` `Yzb3F`): a change made *from the
//! next turn of this step* holds for that step only, so the first later step that starts after it runs on its own
//! profile again, and its Conversation says so.

use orchester_core::{
  id::StepExecutionId,
  resource::StepKey,
  run::{Invocation, ModelChoice, ModelEvent, ModelEventKind, ModelScope, Run},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::profile_choice;

/// A later step that started on its own profile after a change for another step only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackToProfile {
  /// The later step's execution, and when its agent first started.
  pub execution: StepExecutionId,
  pub at: Timestamp,
  pub step: StepKey,
  /// What its profile runs on.
  pub profile: ModelChoice,
  /// The step the change was for.
  pub changed: StepKey,
}

/// For each change for one step among `events` of `run`, the first later step of the pipeline whose agent started
/// after it, when it started on its own profile: no change still held for it then. One line per execution, for the
/// latest such change.
pub(super) fn back_to_profile(
  db: &ProjectDb,
  run: &Run,
  (invocations, events): (&[Invocation], &[ModelEvent]),
) -> Result<Vec<BackToProfile>, StoreError> {
  let order: Vec<&StepKey> = run.snapshot().body().steps().iter().map(|step| &step.key).collect();
  let position = |key: &StepKey| order.iter().position(|step| *step == key);
  let starts = first_starts(run, invocations);
  let mut lines: Vec<BackToProfile> = Vec::new();
  for (index, event) in events.iter().enumerate() {
    let ModelEventKind::Changed {
      scope: ModelScope::Step,
      ..
    } = &event.kind
    else {
      continue;
    };
    let Some(from) = position(&event.step) else {
      continue;
    };
    let next = starts
      .iter()
      .filter(|(_, step, at)| *at > event.at && position(step).is_some_and(|at| at > from))
      .min_by_key(|(.., at)| *at);
    let Some(&(execution, step, at)) = next else {
      continue;
    };
    if canceled_before(events, index, at) || !starts_on_profile(events, step, at) {
      continue;
    }
    let Some(profile) = profile_choice(db, run, step)? else {
      continue;
    };
    lines.retain(|line| line.execution != execution);
    lines.push(BackToProfile {
      execution,
      at,
      step: step.clone(),
      profile,
      changed: event.step.clone(),
    });
  }
  Ok(lines)
}

/// Each execution of `run` whose agent started, with its step and when its first invocation started.
fn first_starts<'a>(run: &'a Run, invocations: &[Invocation]) -> Vec<(StepExecutionId, &'a StepKey, Timestamp)> {
  let mut starts: Vec<(StepExecutionId, &StepKey, Timestamp)> = Vec::new();
  for invocation in invocations {
    let Some(execution) = run.execution(invocation.execution) else {
      continue;
    };
    match starts.iter_mut().find(|(id, ..)| *id == invocation.execution) {
      Some(start) => start.2 = start.2.min(invocation.started_at),
      None => starts.push((invocation.execution, execution.step(), invocation.started_at)),
    }
  }
  starts
}

/// Whether the change at `index` among `events` was canceled before `at`: the next event for its step is a
/// cancelation made before then.
fn canceled_before(events: &[ModelEvent], index: usize, at: Timestamp) -> bool {
  let step = &events[index].step;
  events[index + 1..]
    .iter()
    .find(|event| event.step == *step)
    .is_some_and(|event| matches!(event.kind, ModelEventKind::Canceled { .. }) && event.at < at)
}

/// Whether step `step` started at `at` on its own profile: no change made before then held for it, for it alone or
/// for the rest of the run, unless it was canceled first.
fn starts_on_profile(events: &[ModelEvent], step: &StepKey, at: Timestamp) -> bool {
  !events.iter().enumerate().any(|(index, event)| {
    let ModelEventKind::Changed { also, .. } = &event.kind else {
      return false;
    };
    event.at < at && (event.step == *step || also.contains(step)) && !canceled_before(events, index, at)
  })
}
