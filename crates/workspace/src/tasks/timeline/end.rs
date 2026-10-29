//! How a run's timeline ends: the step the run ended on or stopped at, and the run's end itself.

use orchester_core::{
  integration::RunMerge,
  run::{RunFailure, RunStatus, StepStatus},
};

use super::{Item, ItemKind, RunEnd};
use crate::tasks::detail::RunView;

/// Marks the last step that ran, and its result: the step the run ended on or stopped at.
pub(super) fn mark_ended_on(items: &mut [Item]) {
  let last = items.iter().rev().find_map(|item| match &item.kind {
    ItemKind::Step(head) => head.execution,
    _ => None,
  });
  let Some(last) = last else {
    return;
  };
  for item in items.iter_mut().filter(|item| item.execution == Some(last)) {
    match &mut item.kind {
      ItemKind::Step(head) => head.ended_on = true,
      ItemKind::Result(card) => card.ended_on = true,
      _ => {}
    }
  }
}

pub(super) fn run_end(run: &RunView, merge: Option<&RunMerge>) -> Option<Item> {
  let end = match (run.run.status(), merge) {
    (RunStatus::Succeeded, Some(merge)) => RunEnd::Merged(merge.into.as_str().into()),
    (RunStatus::Succeeded, None) => RunEnd::Ready,
    // It never started, so nothing ran that it could end after.
    (RunStatus::Failed(RunFailure::WorktreeNotCreated(_)), _) => return None,
    (RunStatus::Failed(_), _) => RunEnd::Failed(
      run
        .steps
        .iter()
        .find(|step| step.status == Some(StepStatus::Failed))
        .map_or_else(|| "".into(), |step| step.name.clone()),
    ),
    (RunStatus::Cancelled, _) => RunEnd::Cancelled,
    _ => return None,
  };
  Some(Item {
    key: "end".into(),
    execution: None,
    at: run.finished_at.unwrap_or_else(|| run.run.created_at()),
    kind: ItemKind::End(end),
  })
}
