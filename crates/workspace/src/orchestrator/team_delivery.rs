//! What a team module whose every task is done waits for before its work is on the base branch
//! (`docs/design/local-and-team-parity.md`, "What decides a module's delivery"): its tasks' runs merge into the
//! module's branch on this computer, and the delivery of that branch is its team server's to decide, by an Admin's
//! authorization or the team's automatic merge. The conversation that leads the module asks for it once, after the
//! last merge into the branch made here; a delivery that waits for an Admin, was refused or failed is the person's. A
//! team module keeps no acceptance here, so there is no step before the delivery. Work a teammate's computer merged is
//! not known here and asks for nothing.

use orchester_core::{
  id::ModuleId,
  integration::base::RunBaseSource,
  module::delivery::{DeliveryStep, PersonsDelivery},
  store::{MergeStore, RunStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::{
  tasks::detail::run_store_error,
  team_work::delivery::{Asked, newest_of},
};

/// What the team module `module` of the shared project in `db` waits for once its every task is done. Blocks on
/// SQLite.
pub(super) fn step(db: &ProjectDb, module: ModuleId) -> Result<DeliveryStep, StoreError> {
  let Some(merged) = last_merge(db, module)? else {
    return Ok(DeliveryStep::Delivered);
  };
  Ok(match newest_of(db, module)? {
    Some((asked, came)) if asked >= merged => match came {
      Asked::Delivered => DeliveryStep::Delivered,
      Asked::Waits => DeliveryStep::Person(PersonsDelivery::TeamDecides),
      Asked::Failed => DeliveryStep::Person(PersonsDelivery::TeamRefused),
    },
    // Work reached the branch after the last delivery was asked for, or none was.
    Some(_) | None => DeliveryStep::Deliver,
  })
}

/// When a team run that started on the branch of `module` was last merged into it here; `None` while none was.
fn last_merge(db: &ProjectDb, module: ModuleId) -> Result<Option<Timestamp>, StoreError> {
  let mut last = None;
  for task in db.team_task_ids()? {
    for run in db.runs_of_task(task).map_err(run_store_error)?.iter().flatten() {
      if run.base_source() == (RunBaseSource::Module { module }) {
        last = last.max(db.run_merge(run.id())?.map(|merge| merge.merged_at));
      }
    }
  }
  Ok(last)
}
