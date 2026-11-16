//! Recording a new run: a local task's, which must be queued, ready and still have its definition; and a run with the
//! execution inputs it is created with ([`ProjectDb::create_run_with_inputs`]), kept in the same transaction. A team
//! run needs its task's identity only: the server accepted its start, and its inputs carry the task as it was then.

use orchester_core::{
  id::{IdempotencyKey, RunId},
  run::{
    NewRun, Run,
    inputs::{InputsOrigin, RunInputs},
  },
  sharing::TaskRevision,
  task::identity::TaskOwner,
  time::Timestamp,
};
use rusqlite::Transaction;

use super::{insert_event, insert_run, load::load_runs, run_number};
use crate::{
  RunStoreError, StoreError,
  project_db::{
    ProjectDb, organization, run_inputs, shared_tasks, task_identities,
    tasks::{has_revision, is_queued, snapshot_new_run},
  },
};

impl ProjectDb {
  /// [`orchester_core::store::RunStore::create_run`] for a task the caller found queued: the task must still be
  /// queued when the run is recorded, in the same transaction. A person may have taken it back to the drafts since the
  /// caller checked it (a held group's task), and a draft never has a run. With `revision`, the task must also still
  /// be at that revision, read in the same transaction: a task edited since a person confirmed its run is refused
  /// ([`RunStoreError::TaskChangedMeanwhile`]).
  pub fn create_run_of_queued_task(
    &mut self,
    key: IdempotencyKey,
    new: NewRun,
    now: Timestamp,
    revision: Option<TaskRevision>,
  ) -> Result<Run, RunStoreError> {
    self.record_run(key, new, now, Some(revision), None)
  }

  /// Records the run with the `inputs` it executes with, in one transaction; nothing is recorded when the inputs do
  /// not match it. The run's snapshot must pin exactly the versions the inputs carry. Local inputs are a local task's,
  /// recorded as [`Self::create_run_of_queued_task`] records it. Team inputs are a team task's whose identity
  /// [`Self::record_team_task`] recorded, and need no definition of it here; their operation starts one run only, so
  /// a retry with another key is refused.
  pub fn create_run_with_inputs(
    &mut self,
    key: IdempotencyKey,
    new: NewRun,
    inputs: &RunInputs,
    (now, revision): (Timestamp, Option<TaskRevision>),
  ) -> Result<Run, RunStoreError> {
    if !inputs.pinned_by(&new.snapshot) {
      return Err(RunStoreError::InputsMismatch {
        task: new.task,
        reason: "the run's snapshot does not pin the inputs' versions",
      });
    }
    match inputs.bundle().origin() {
      InputsOrigin::Local => self.record_run(key, new, now, Some(revision), Some(inputs)),
      InputsOrigin::Team(_) => self.record_team_run(key, new, now, inputs),
    }
  }

  /// Records a local task's run; `queued` checks the task is still queued and, with a revision, still at it.
  pub(super) fn record_run(
    &mut self,
    key: IdempotencyKey,
    new: NewRun,
    now: Timestamp,
    queued: Option<Option<TaskRevision>>,
    inputs: Option<&RunInputs>,
  ) -> Result<Run, RunStoreError> {
    let key = key.to_string();
    let project = self.project.key().clone();
    let transaction = self.connection.transaction()?;
    if let Some(existing) = load_runs(&transaction, "WHERE idempotency_key = ?1", rusqlite::params![key])?.pop() {
      return existing.map_err(|unreadable| unreadable.error);
    }
    if let Some(revision) = queued {
      if !is_queued(&transaction, new.task)? {
        return Err(RunStoreError::TaskNotQueued(new.task));
      }
      if let Some(revision) = revision
        && !has_revision(&transaction, new.task, revision)?
      {
        return Err(RunStoreError::TaskChangedMeanwhile(new.task));
      }
    }
    if shared_tasks::is_not_shared(&transaction, new.task)? {
      return Err(StoreError::NotSharedTask(new.task).into());
    }
    organization::check_readiness(&transaction, new.task)?;
    super::archived::refuse(&transaction, &new.snapshot)?;
    let run = insert_new_run(&transaction, &key, new, now)?;
    snapshot_new_run(&transaction, run.id(), run.task(), &project)?;
    if let Some(inputs) = inputs {
      run_inputs::insert(&transaction, run.id(), inputs)?;
    }
    transaction.commit()?;
    Ok(run)
  }

  fn record_team_run(
    &mut self,
    key: IdempotencyKey,
    new: NewRun,
    now: Timestamp,
    inputs: &RunInputs,
  ) -> Result<Run, RunStoreError> {
    let key = key.to_string();
    let transaction = self.connection.transaction()?;
    if let Some(existing) = load_runs(&transaction, "WHERE idempotency_key = ?1", rusqlite::params![key])?.pop() {
      return existing.map_err(|unreadable| unreadable.error);
    }
    let InputsOrigin::Team(accepted) = inputs.bundle().origin() else {
      return Err(RunStoreError::InputsMismatch {
        task: new.task,
        reason: "the inputs are not a team's",
      });
    };
    let identity = task_identities::load(&transaction, new.task)?.ok_or(StoreError::TaskNotFound(new.task))?;
    if !matches!(identity.owner, TaskOwner::Team { task, .. } if task == accepted.task) {
      return Err(RunStoreError::InputsMismatch {
        task: new.task,
        reason: "the task is not the team task the inputs were accepted for",
      });
    }
    if run_inputs::run_of_operation(&transaction, accepted.operation)?.is_some() {
      return Err(RunStoreError::InputsMismatch {
        task: new.task,
        reason: "the operation that accepted the inputs started another run",
      });
    }
    let run = insert_new_run(&transaction, &key, new, now)?;
    run_inputs::insert(&transaction, run.id(), inputs)?;
    transaction.commit()?;
    Ok(run)
  }
}

/// Numbers and stores the new run `new` with its creation event.
fn insert_new_run(transaction: &Transaction<'_>, key: &str, new: NewRun, now: Timestamp) -> Result<Run, RunStoreError> {
  let number: i64 = transaction.query_row("SELECT next FROM counters WHERE name = 'run'", [], |row| row.get(0))?;
  let (run, created) = Run::create(RunId::generate(), run_number(number)?, new, now)?;
  insert_run(transaction, &run, key)?;
  insert_event(transaction, &created)?;
  transaction.execute("UPDATE counters SET next = ?1 WHERE name = 'run'", [number + 1])?;
  Ok(run)
}
