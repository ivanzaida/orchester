//! The cache of a shared project's modules (`docs/architecture/server.md`, "Modules"): the server's modules with their
//! revision, the module each cached task is in, and the moves the server kept, written only from what the server
//! confirmed. A local module that became the server's module of its name when the project was shared or joined takes
//! that module's id here. *Make local* keeps them all as the project's own modules.

use orchester_core::{
  id::{ModuleId, TaskId},
  module::{MembershipChange, Module},
  sharing::TaskRevision,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{ProjectDb, modules, sharing};
use crate::{
  StoreError,
  rows::{corrupt, parse, tokens_column},
};

/// A module as the server confirmed it, with its revision there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedModule {
  pub module: Module,
  pub revision: TaskRevision,
}

/// A move of a task the server kept, by its sequence there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CachedModuleChange {
  pub sequence: u64,
  pub change: MembershipChange,
}

impl ProjectDb {
  /// Takes the server project's `modules` in while the project is shared or joins, before it is linked: a local module
  /// that has the name of one of them, without regard to case, became that module on the server when it was uploaded,
  /// and takes its id, in its tasks and its moves too; every module is written as the server has it.
  pub fn adopt_shared_modules(&mut self, modules: &[CachedModule]) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    if sharing::link(&transaction)?.is_some() {
      return Err(StoreError::AlreadyShared);
    }
    // The ids change in the modules, the tasks and the moves together; they refer to each other again at commit.
    transaction.pragma_update(None, "defer_foreign_keys", true)?;
    for cached in modules {
      let shared = cached.module.id();
      let local: Option<String> = transaction
        .query_row(
          "SELECT id FROM modules WHERE normalized = ?1 AND id <> ?2",
          params![modules::normalized(cached.module.name()), shared.to_string()],
          |row| row.get(0),
        )
        .optional()?;
      if let Some(local) = local {
        rekey(&transaction, parse("module id", &local)?, shared)?;
      }
    }
    write(&transaction, modules)?;
    transaction.commit()?;
    Ok(())
  }

  /// Writes the shared project's `modules` as the server lists them now.
  pub fn cache_shared_modules(&mut self, modules: &[CachedModule]) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    if sharing::link(&transaction)?.is_none() {
      return Err(StoreError::NotShared);
    }
    write(&transaction, modules)?;
    transaction.commit()?;
    Ok(())
  }

  /// Keeps the server's moves, oldest first, that the cache does not have yet. A move of a task the cache does not hold
  /// yet (made since its tasks were read) and every later move wait for the next read: [`Self::last_module_change`]
  /// stays before them.
  pub fn cache_module_changes(&mut self, changes: &[CachedModuleChange]) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    if sharing::link(&transaction)?.is_none() {
      return Err(StoreError::NotShared);
    }
    for cached in changes {
      let change = cached.change;
      let held: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1)",
        [change.task.to_string()],
        |row| row.get(0),
      )?;
      if !held {
        break;
      }
      transaction.execute(
        "INSERT OR IGNORE INTO module_changes (task_id, from_module, to_module, actor, changed_at, server_sequence) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
          change.task.to_string(),
          change.from.map(|id| id.to_string()),
          change.to.map(|id| id.to_string()),
          modules::actor_text(change.actor),
          change.at.unix_millis(),
          tokens_column("module change sequence", cached.sequence)?,
        ],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }

  /// The last of the server's moves the cache holds; 0 for none.
  pub fn last_module_change(&self) -> Result<u64, StoreError> {
    let last: i64 = self.connection.query_row(
      "SELECT COALESCE(MAX(server_sequence), 0) FROM module_changes",
      [],
      |row| row.get(0),
    )?;
    u64::try_from(last).map_err(|error| corrupt("module change sequence", error))
  }

  /// The server's revision of module `id` as last cached; `None` for a local project's module.
  pub fn module_revision(&self, id: ModuleId) -> Result<Option<TaskRevision>, StoreError> {
    let revision: Option<Option<i64>> = self
      .connection
      .query_row("SELECT revision FROM modules WHERE id = ?1", [id.to_string()], |row| {
        row.get(0)
      })
      .optional()?;
    revision
      .flatten()
      .map(|revision| {
        u64::try_from(revision)
          .ok()
          .and_then(|revision| TaskRevision::try_from(revision).ok())
          .ok_or_else(|| corrupt("module revision", revision))
      })
      .transpose()
  }
}

/// Puts the cached `task` in `module`, or in none, as the server has it; the server keeps the move.
pub(super) fn write_task_module(
  connection: &Connection,
  task: TaskId,
  module: Option<ModuleId>,
) -> Result<(), StoreError> {
  match module {
    Some(module) => connection.execute(
      "INSERT INTO task_modules (task_id, module_id) VALUES (?1, ?2) \
       ON CONFLICT (task_id) DO UPDATE SET module_id = excluded.module_id",
      params![task.to_string(), module.to_string()],
    )?,
    None => connection.execute("DELETE FROM task_modules WHERE task_id = ?1", [task.to_string()])?,
  };
  Ok(())
}

/// The local module `local` becomes the server's `shared`: its tasks, its moves and its base branch, which stays on
/// this computer, name `shared` from now on.
fn rekey(transaction: &Transaction<'_>, local: ModuleId, shared: ModuleId) -> Result<(), StoreError> {
  if local == shared {
    return Ok(());
  }
  let ids = params![local.to_string(), shared.to_string()];
  transaction.execute("UPDATE task_modules SET module_id = ?2 WHERE module_id = ?1", ids)?;
  transaction.execute("UPDATE module_changes SET from_module = ?2 WHERE from_module = ?1", ids)?;
  transaction.execute("UPDATE module_changes SET to_module = ?2 WHERE to_module = ?1", ids)?;
  let taken: bool = transaction.query_row(
    "SELECT EXISTS(SELECT 1 FROM modules WHERE id = ?1)",
    [shared.to_string()],
    |row| row.get(0),
  )?;
  if taken {
    transaction.execute(
      "UPDATE modules SET base_branch = COALESCE(base_branch, (SELECT base_branch FROM modules WHERE id = ?1)) \
       WHERE id = ?2",
      ids,
    )?;
    transaction.execute(
      "UPDATE OR IGNORE base_branch_move_modules SET module_id = ?2 WHERE module_id = ?1",
      ids,
    )?;
    transaction.execute("DELETE FROM modules WHERE id = ?1", [local.to_string()])?;
  } else {
    transaction.execute("UPDATE modules SET id = ?2 WHERE id = ?1", ids)?;
  }
  Ok(())
}

pub(super) fn write(transaction: &Transaction<'_>, cached: &[CachedModule]) -> Result<(), StoreError> {
  // A name can pass from one module to another between two reads: each module's name is set aside first.
  for module in cached {
    transaction.execute(
      "UPDATE modules SET normalized = '#' || id WHERE id = ?1",
      [module.module.id().to_string()],
    )?;
  }
  for CachedModule { module, revision } in cached {
    let details = module.details();
    transaction.execute(
      "INSERT INTO modules (id, name, normalized, description, start_date, target_date, state, created_at, revision) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) ON CONFLICT (id) DO UPDATE SET name = excluded.name, \
       normalized = excluded.normalized, description = excluded.description, start_date = excluded.start_date, \
       target_date = excluded.target_date, state = excluded.state, created_at = excluded.created_at, \
       revision = excluded.revision",
      params![
        module.id().to_string(),
        details.name().as_str(),
        modules::normalized(details.name()),
        details.description(),
        details.start().map(|day| day.to_string()),
        details.target().map(|day| day.to_string()),
        modules::state_text(details.state()),
        module.created_at().unix_millis(),
        tokens_column("module revision", revision.get())?,
      ],
    )?;
  }
  Ok(())
}
