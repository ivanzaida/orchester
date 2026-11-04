//! What a run's model chip changed (ORC-322, the event lines of `08f · Model and effort on a run` `Yzb3F`): a quiet
//! line for each change of the run's model or effort and each cancelation, among the items of the step it was made for
//! — of its execution whose agent last started before it — or among the items that wait for a step when its agent had
//! not started yet. A change made while the step's turn runs says it waits, and offers to cancel it, until that turn
//! ends. After a change for one step only, the next step that starts on its own profile says so as it starts.

use orchester_core::{
  id::StepExecutionId,
  resource::StepKey,
  run::{Answerer, ModelEventKind},
  time::Timestamp,
};

use super::{Item, ItemKind};
use crate::tasks::{detail::RunView, run_model::waiting_change};

/// One change of the run's model or effort, or its cancelation, as its line shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelLine {
  pub by: Answerer,
  /// The step it was made for.
  pub step: StepKey,
  pub kind: ModelEventKind,
  /// The change waits for the turn that runs to end: its line offers *Cancel change*.
  pub waits: bool,
}

/// The lines of `run`'s model events that belong to `execution` (`None`: to no step), and the line of a later step
/// that started on its own profile again after a change for one step, at that start.
pub(super) fn items(run: &RunView, execution: Option<StepExecutionId>) -> Vec<Item> {
  let events = &run.models.events;
  let waits = |index: usize, step: &StepKey| {
    waiting_change(&run.run, &run.invocations, events, step).is_some_and(|(waiting, _)| waiting == index)
  };
  let mut items: Vec<Item> = events
    .iter()
    .enumerate()
    .filter(|(_, event)| made_in(run, &event.step, event.at) == execution)
    .map(|(index, event)| Item {
      key: format!("model-{index}").into(),
      execution,
      at: event.at,
      kind: ItemKind::Model(ModelLine {
        by: event.by,
        step: event.step.clone(),
        kind: event.kind.clone(),
        waits: waits(index, &event.step),
      }),
    })
    .collect();
  items.extend(
    run
      .models
      .back
      .iter()
      .filter(|back| Some(back.execution) == execution)
      .map(|back| Item {
        key: format!("model-back-{}", back.execution).into(),
        execution,
        at: back.at,
        kind: ItemKind::BackToProfile(back.clone()),
      }),
  );
  items
}

/// The execution of step `step` whose agent last started at or before `at`.
fn made_in(run: &RunView, step: &StepKey, at: Timestamp) -> Option<StepExecutionId> {
  run
    .invocations
    .iter()
    .filter(|invocation| invocation.started_at <= at)
    .filter(|invocation| {
      run
        .run
        .execution(invocation.execution)
        .is_some_and(|execution| execution.step() == step)
    })
    .max_by_key(|invocation| invocation.started_at)
    .map(|invocation| invocation.execution)
}
