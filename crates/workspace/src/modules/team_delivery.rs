//! What a team module whose every task is done waits for before its work is on the base branch
//! (`docs/design/local-and-team-parity.md`; `docs/plan/orchestrator-autonomy/OA-20-…`, `OA-32-…`): the rule of a
//! local module ([`DeliveryFacts::step`]), read from what this computer keeps of a team module. Its tasks' runs merge
//! into the module's branch here; its acceptance is a task Orchester made on the team server in the module, kept here
//! as the module's (`module_acceptances`), and its acceptance pipeline and *Deliver when accepted* are this computer's
//! choice (`team_module_acceptance`), which the team server does not keep. Its delivery is the team server's to decide,
//! by an Admin's authorization or the team's automatic merge: once asked for after the last merge into the branch made
//! here, what it came to is the module's step. Work a teammate's computer merged is not known here and asks for
//! nothing.

use std::path::Path;

use orchester_core::{
  id::{ModuleId, TaskId},
  integration::base::RunBaseSource,
  module::delivery::{Acceptance, AcceptanceChoice, DeliveryFacts, DeliveryStep, PersonsDelivery},
  run::TaskPhase,
  store::{MergeStore, RunStore},
  task::organization::WorkflowStatus,
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use super::delivery::acceptance_by;
use crate::{
  messages::{self, Message},
  tasks::{detail::run_store_error, recorded},
  team_work::delivery::{Asked, newest_of},
};

/// Where a team module stands on its way to its base branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeamDelivery {
  /// What the rule reads of it.
  pub facts: DeliveryFacts,
  /// What it waits for once its every task is done.
  pub step: DeliveryStep,
  /// Work was merged into its branch on this computer.
  pub merged: bool,
}

/// Where the team module `module` of the shared project in `db` stands; `status_of` gives the workflow status the team
/// server holds a task in, `None` while it is not known (its latest acceptance's, read by the caller from its server or
/// its last reading). Blocks on SQLite.
pub fn read(
  db: &ProjectDb,
  module: ModuleId,
  status_of: &dyn Fn(TaskId) -> Option<WorkflowStatus>,
) -> Result<TeamDelivery, StoreError> {
  let choice = db.team_module_acceptance(module)?;
  let acceptances = db.module_acceptances(module)?;
  let is_acceptance = |task: TaskId| acceptances.iter().any(|(acceptance, _)| *acceptance == task);
  // When work last reached the module's branch here: by any task, and by a task that is no acceptance.
  let (mut merged, mut work_merged) = (None, None);
  for task in db.team_task_ids()? {
    let last = last_merge(db, task, module)?;
    merged = merged.max(last);
    if !is_acceptance(task) {
      work_merged = work_merged.max(last);
    }
  }
  let acceptance = match acceptances.last() {
    None => Acceptance::None,
    Some(&(task, at)) => acceptance_of(
      db,
      (task, status_of(task)),
      work_merged.is_some_and(|merged| merged > at),
    )?,
  };
  // What the delivery asked for after the last merge into the branch came to; one asked for before it decides nothing.
  let asked = newest_of(db, module)?
    .filter(|(asked, _)| merged.is_some_and(|merged| *asked >= merged))
    .map(|(_, came)| came);
  let facts = DeliveryFacts {
    own_branch: true,
    acceptance_pipeline: choice.acceptance.is_some(),
    deliver_when_accepted: choice.deliver_when_accepted,
    acceptances: acceptances.len(),
    acceptance,
    // Nothing merged here holds nothing to deliver.
    delivered: merged.is_none() || asked == Some(Asked::Delivered),
  };
  let step = match asked {
    Some(Asked::Waits) => DeliveryStep::Person(PersonsDelivery::TeamDecides),
    Some(Asked::Failed) => DeliveryStep::Person(PersonsDelivery::TeamRefused),
    Some(Asked::Delivered) | None => facts.step(),
  };
  Ok(TeamDelivery {
    facts,
    step,
    merged: merged.is_some(),
  })
}

/// What this computer chose for the acceptance of the team module `module` of the project at `root`. Blocks on SQLite.
pub fn choice_in(root: &Path, module: ModuleId) -> Result<AcceptanceChoice, Message> {
  let stored = |error: StoreError| messages::store_error(&error);
  ProjectDb::open_read_only(root)
    .and_then(|db| db.team_module_acceptance(module))
    .map_err(stored)
}

/// Keeps `choice` for the acceptance of the team module `module` of the project at `root`; *Deliver when accepted*
/// goes off without an acceptance pipeline, which it cannot be without. Nothing is sent to the team server. Blocks on
/// SQLite.
pub fn choose_in(root: &Path, module: ModuleId, choice: AcceptanceChoice) -> Result<AcceptanceChoice, Message> {
  let kept = choice.kept();
  ProjectDb::open(root)
    .and_then(|mut db| db.set_team_module_acceptance(module, kept))
    .map_err(|error| messages::store_error(&error))?;
  Ok(kept)
}

/// When a run of team task `task` that started on the branch of `module` was last merged into it here; `None` while
/// none was.
fn last_merge(db: &ProjectDb, task: TaskId, module: ModuleId) -> Result<Option<Timestamp>, StoreError> {
  let mut last = None;
  for run in db.runs_of_task(task).map_err(run_store_error)?.iter().flatten() {
    if run.base_source() == (RunBaseSource::Module { module }) {
      last = last.max(db.run_merge(run.id())?.map(|merge| merge.merged_at));
    }
  }
  Ok(last)
}

/// Where the acceptance `task`, which the team server holds in `status`, stands; `stale`: work was merged into the
/// module's branch after it was made.
fn acceptance_of(
  db: &ProjectDb,
  (task, status): (TaskId, Option<WorkflowStatus>),
  stale: bool,
) -> Result<Acceptance, StoreError> {
  let runs = db.runs_of_task(task).map_err(run_store_error)?;
  let phase = match runs.iter().rev().find_map(|listed| listed.as_ref().ok()) {
    Some(run) => {
      let merge = db.run_merge(run.id())?;
      recorded::task_of(db, task)?.map(|recorded| TaskPhase::of(&recorded, Some(run), merge.as_ref()))
    }
    None => None,
  };
  Ok(acceptance_by(status, phase, stale))
}
