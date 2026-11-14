//! A module's own branch, its deliveries and its acceptances (`orchester_core::module::delivery`,
//! `docs/plan/orchestrator-autonomy/OA-17-…`, `OA-20-…`): `modules.own_branch` with the acceptance pipeline and
//! *Deliver when accepted*, kept on this computer as a module's base branch is (a shared module's are never sent);
//! `module_deliveries`, each merge of a module's branch into its base branch; and `module_acceptances`, the tasks
//! Orchester made to accept a module.

use orchester_core::{
  id::{ModuleId, TaskId},
  module::{
    Module, ModuleError,
    delivery::{ModuleDelivered, OwnBranch},
  },
  run::BranchName,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::{ProjectDb, modules};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

impl ProjectDb {
  /// Gives module `id` the own branch `own`, or takes it away. Runs that started keep the base they recorded; the
  /// branch in the repository is neither made nor removed here.
  pub fn set_module_own_branch(&mut self, id: ModuleId, own: Option<&OwnBranch>) -> Result<Module, StoreError> {
    let transaction = self.connection.transaction()?;
    let module = modules::load(&transaction, id)?.ok_or(ModuleError::NotFound(id))?;
    transaction.execute(
      "UPDATE modules SET own_branch = ?2, acceptance_pipeline = ?3, deliver_when_accepted = ?4 WHERE id = ?1",
      params![
        id.to_string(),
        own.map(|own| own.branch.as_str()),
        own.and_then(|own| own.acceptance).map(|pipeline| pipeline.to_string()),
        own.is_some_and(|own| own.deliver_when_accepted),
      ],
    )?;
    transaction.commit()?;
    Ok(module.with_own_branch(own.cloned()))
  }

  /// Keeps a delivery of a module that merged.
  pub fn keep_module_delivery(&mut self, delivered: &ModuleDelivered) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO module_deliveries (module_id, head, into_branch, merge_commit, at) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        delivered.module.to_string(),
        delivered.head.as_str(),
        delivered.into.as_str(),
        delivered.merge.as_str(),
        delivered.at.unix_millis(),
      ],
    )?;
    Ok(())
  }

  /// The latest delivery of module `module`; `None` while it was never delivered.
  pub fn module_delivery(&self, module: ModuleId) -> Result<Option<ModuleDelivered>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT head, into_branch, merge_commit, at FROM module_deliveries WHERE module_id = ?1
         ORDER BY at DESC, id DESC LIMIT 1",
        [module.to_string()],
        |row| {
          Ok((
            [
              row.get::<_, String>(0)?,
              row.get::<_, String>(1)?,
              row.get::<_, String>(2)?,
            ],
            row.get::<_, i64>(3)?,
          ))
        },
      )
      .optional()?;
    row
      .map(|([head, into, merge], at)| {
        Ok(ModuleDelivered {
          module,
          head: parse("module delivery head", &head)?,
          into: BranchName::new(&into).map_err(|error| corrupt("module delivery branch", error))?,
          merge: parse("module delivery merge", &merge)?,
          at: Timestamp::from_unix_millis(at),
        })
      })
      .transpose()
  }

  /// Keeps that `task` is an acceptance of module `module`, made at `at`.
  pub fn keep_module_acceptance(&mut self, module: ModuleId, task: TaskId, at: Timestamp) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO module_acceptances (task_id, module_id, at) VALUES (?1, ?2, ?3)",
      params![task.to_string(), module.to_string(), at.unix_millis()],
    )?;
    Ok(())
  }

  /// Every task Orchester made to accept a module.
  pub fn acceptance_task_ids(&self) -> Result<Vec<TaskId>, StoreError> {
    let mut statement = self.connection.prepare("SELECT task_id FROM module_acceptances")?;
    let rows = statement
      .query_map([], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    rows.iter().map(|task| parse("acceptance task id", task)).collect()
  }

  /// The acceptance tasks of module `module` with when each was made, oldest first.
  pub fn module_acceptances(&self, module: ModuleId) -> Result<Vec<(TaskId, Timestamp)>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT task_id, at FROM module_acceptances WHERE module_id = ?1 ORDER BY at, rowid")?;
    let rows = statement
      .query_map([module.to_string()], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(task, at)| Ok((parse("acceptance task id", &task)?, Timestamp::from_unix_millis(at))))
      .collect()
  }
}

/// A module's own branch as its row's columns hold it.
pub(super) fn parse_own(
  branch: Option<&str>,
  acceptance: Option<&str>,
  deliver_when_accepted: bool,
) -> Result<Option<OwnBranch>, StoreError> {
  let Some(branch) = branch else {
    return Ok(None);
  };
  Ok(Some(OwnBranch {
    branch: BranchName::new(branch).map_err(|error| corrupt("module own branch", error))?,
    acceptance: acceptance
      .map(|pipeline| parse("module acceptance pipeline", pipeline))
      .transpose()?,
    deliver_when_accepted,
  }))
}
