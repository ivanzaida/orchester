//! What a run's permissions menu changed (ORC-319, `Event lines` `BaB32` in 15f `CimC4`): a quiet line for each of the
//! run's permission events — approving everything, stopping it, a revoked rule, automation's refused asking to widen
//! the run — among the items of the step whose agent last started before it, or among the items that wait for a step
//! when none had started, as the run's switch to Manual ([`super::manual_switch`]).

use orchester_core::{
  id::StepExecutionId,
  run::{Answerer, PermissionEventKind},
};

use super::{Item, ItemKind, manual_switch::switched_in};
use crate::tasks::detail::RunView;

/// One permission event as its line shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionLine {
  pub by: Answerer,
  pub kind: PermissionEventKind,
  /// The line of the approving everything that still lasts: it offers *Stop*.
  pub lasting: bool,
}

/// The lines of `run`'s permission events that belong to `execution` (`None`: to no step).
pub(super) fn items(run: &RunView, execution: Option<StepExecutionId>) -> Vec<Item> {
  let approvals = &run.approvals;
  let lasting = approvals
    .approves_everything()
    .then(|| {
      approvals
        .events
        .iter()
        .rposition(|event| matches!(event.kind, PermissionEventKind::Everything { .. }))
    })
    .flatten();
  approvals
    .events
    .iter()
    .enumerate()
    .filter(|(_, event)| switched_in(run, event.at) == execution)
    .map(|(index, event)| Item {
      key: format!("permission-{index}").into(),
      execution,
      at: event.at,
      kind: ItemKind::Permission(PermissionLine {
        by: event.by,
        kind: event.kind.clone(),
        lasting: lasting == Some(index),
      }),
    })
    .collect()
}
