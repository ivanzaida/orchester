//! A run switched to Manual for itself (`docs/design/approval-modes.md`, `Event · switched to Manual` `ARwiU` in 04c):
//! its row *Switched to Manual for this run*, among the items of the step whose agent last started before the switch,
//! or among the items that wait for a step when none had started.

use std::sync::Arc;

use orchester_core::{
  id::StepExecutionId,
  run::{Answerer, EffectiveMode, ManualBecause},
  time::Timestamp,
};

use super::{Item, ItemKind};
use crate::tasks::detail::RunView;

/// The row's key: a run is switched once.
const KEY: &str = "switched-to-manual";

/// Who switched the run, and whose profile stays on Autopilot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchedToManual {
  pub by: Answerer,
  /// The agent of the step it was switched in, while the run is Manual for itself alone: its profile still says
  /// Autopilot.
  pub agent: Option<Arc<str>>,
}

/// The switch's item when it belongs to `execution` (`None`: to no step).
pub(super) fn item(run: &RunView, execution: Option<StepExecutionId>) -> Option<Item> {
  let switch = run.approvals.switched?;
  if switched_in(run, switch.at) != execution {
    return None;
  }
  let for_itself = run.approvals.mode == EffectiveMode::Manual(ManualBecause::Run);
  let agent = execution
    .filter(|_| for_itself)
    .and_then(|execution| run.step_of(execution))
    .and_then(|step| step.agent.clone());
  Some(Item {
    key: KEY.into(),
    execution,
    at: switch.at,
    kind: ItemKind::SwitchedToManual(SwitchedToManual { by: switch.by, agent }),
  })
}

/// The step execution whose agent last started at or before `at`.
fn switched_in(run: &RunView, at: Timestamp) -> Option<StepExecutionId> {
  run
    .invocations
    .iter()
    .filter(|invocation| invocation.started_at <= at)
    .max_by_key(|invocation| invocation.started_at)
    .map(|invocation| invocation.execution)
}
