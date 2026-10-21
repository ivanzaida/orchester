//! `modules`, `task_modules` and `module_changes`: a project's modules, the module each task is in, and every move
//! of a task into or out of one (`orchester_core::module`). Modules are made on this computer only: a shared
//! project's task list and its tasks change on the server, which has no modules.

use orchester_core::{
  id::{ModuleId, TaskId},
  module::{MembershipChange, Module, ModuleDetails, ModuleError, ModuleState},
  name::Name,
  task::organization::ChangeActor,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

const SELECT_MODULES: &str = "SELECT id, name, description, start_date, target_date, state, created_at FROM modules";

impl ProjectDb {
  /// The project's modules, oldest first.
  pub fn modules(&self) -> Result<Vec<Module>, StoreError> {
    let mut statement = self
      .connection
      .prepare(&format!("{SELECT_MODULES} ORDER BY created_at, id"))?;
    let rows = statement
      .query_map([], ModuleRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(ModuleRow::into_module).collect()
  }

  pub fn module(&self, id: ModuleId) -> Result<Option<Module>, StoreError> {
    load(&self.connection, id)
  }

  pub fn create_module(&mut self, details: ModuleDetails, at: Timestamp) -> Result<Module, StoreError> {
    let transaction = self.connection.transaction()?;
    super::sharing::ensure_local_project(&transaction)?;
    unique_name(&transaction, details.name(), None)?;
    let module = Module::new(ModuleId::generate(), details, at);
    let details = module.details();
    transaction.execute(
      "INSERT INTO modules (id, name, normalized, description, start_date, target_date, state, created_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
      params![
        module.id().to_string(),
        details.name().as_str(),
        normalized(details.name()),
        details.description(),
        details.start().map(|day| day.to_string()),
        details.target().map(|day| day.to_string()),
        state_text(details.state()),
        at.unix_millis(),
      ],
    )?;
    transaction.commit()?;
    Ok(module)
  }

  /// Replaces the details of module `id`, read as `base`, with `details`; its tasks and history stay. Details that
  /// are no longer `base` are not replaced ([`ModuleError::ChangedMeanwhile`]).
  pub fn update_module(
    &mut self,
    id: ModuleId,
    base: &ModuleDetails,
    details: ModuleDetails,
  ) -> Result<Module, StoreError> {
    let transaction = self.connection.transaction()?;
    super::sharing::ensure_local_project(&transaction)?;
    let module = load(&transaction, id)?.ok_or(ModuleError::NotFound(id))?;
    if module.details() != base {
      return Err(ModuleError::ChangedMeanwhile.into());
    }
    unique_name(&transaction, details.name(), Some(id))?;
    transaction.execute(
      "UPDATE modules SET name = ?2, normalized = ?3, description = ?4, start_date = ?5, target_date = ?6, \
       state = ?7 WHERE id = ?1",
      params![
        id.to_string(),
        details.name().as_str(),
        normalized(details.name()),
        details.description(),
        details.start().map(|day| day.to_string()),
        details.target().map(|day| day.to_string()),
        state_text(details.state()),
      ],
    )?;
    transaction.commit()?;
    Ok(module.with_details(details))
  }

  /// Puts `task` in `module`, or in none, and keeps the move; whether the task moved. A task that is in
  /// `module` already stays and nothing is kept.
  pub fn set_task_module(
    &mut self,
    task: TaskId,
    module: Option<ModuleId>,
    actor: ChangeActor,
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    super::sharing::ensure_local_task(&transaction, task)?;
    let exists: bool = transaction.query_row(
      "SELECT EXISTS(SELECT 1 FROM tasks WHERE id = ?1)",
      [task.to_string()],
      |row| row.get(0),
    )?;
    if !exists {
      return Err(StoreError::TaskNotFound(task));
    }
    if let Some(module) = module
      && load(&transaction, module)?.is_none()
    {
      return Err(ModuleError::NotFound(module).into());
    }
    let from = task_module(&transaction, task)?;
    if from == module {
      return Ok(false);
    }
    match module {
      Some(module) => transaction.execute(
        "INSERT INTO task_modules (task_id, module_id) VALUES (?1, ?2) \
         ON CONFLICT (task_id) DO UPDATE SET module_id = excluded.module_id",
        params![task.to_string(), module.to_string()],
      )?,
      None => transaction.execute("DELETE FROM task_modules WHERE task_id = ?1", [task.to_string()])?,
    };
    transaction.execute(
      "INSERT INTO module_changes (task_id, from_module, to_module, actor, changed_at) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        task.to_string(),
        from.map(|id| id.to_string()),
        module.map(|id| id.to_string()),
        actor_text(actor),
        at.unix_millis(),
      ],
    )?;
    transaction.commit()?;
    Ok(true)
  }

  /// Every move of a task into or out of `module`, oldest first.
  pub fn module_changes(&self, module: ModuleId) -> Result<Vec<MembershipChange>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT task_id, from_module, to_module, actor, changed_at FROM module_changes \
       WHERE from_module = ?1 OR to_module = ?1 ORDER BY sequence",
    )?;
    let rows = statement
      .query_map([module.to_string()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, Option<String>>(1)?,
          row.get::<_, Option<String>>(2)?,
          row.get::<_, String>(3)?,
          row.get::<_, i64>(4)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(task, from, to, actor, at)| {
        Ok(MembershipChange {
          task: parse("task id", &task)?,
          from: from.as_deref().map(|id| parse("module id", id)).transpose()?,
          to: to.as_deref().map(|id| parse("module id", id)).transpose()?,
          actor: parse_actor(&actor)?,
          at: Timestamp::from_unix_millis(at),
        })
      })
      .collect()
  }
}

fn load(connection: &Connection, id: ModuleId) -> Result<Option<Module>, StoreError> {
  connection
    .query_row(
      &format!("{SELECT_MODULES} WHERE id = ?1"),
      [id.to_string()],
      ModuleRow::read,
    )
    .optional()?
    .map(ModuleRow::into_module)
    .transpose()
}

fn task_module(connection: &Connection, task: TaskId) -> Result<Option<ModuleId>, StoreError> {
  connection
    .query_row(
      "SELECT module_id FROM task_modules WHERE task_id = ?1",
      [task.to_string()],
      |row| row.get::<_, String>(0),
    )
    .optional()?
    .map(|id| parse("module id", &id))
    .transpose()
}

/// Names are distinct without regard to case, as tags are.
fn normalized(name: &Name) -> String {
  name.as_str().to_lowercase()
}

fn unique_name(connection: &Connection, name: &Name, except: Option<ModuleId>) -> Result<(), StoreError> {
  let existing: Option<String> = connection
    .query_row(
      "SELECT id FROM modules WHERE normalized = ?1",
      [normalized(name)],
      |row| row.get(0),
    )
    .optional()?;
  if let Some(id) = existing
    && Some(parse("module id", &id)?) != except
  {
    return Err(ModuleError::DuplicateName.into());
  }
  Ok(())
}

const fn state_text(state: ModuleState) -> &'static str {
  match state {
    ModuleState::Open => "open",
    ModuleState::Closed => "closed",
  }
}

const fn actor_text(actor: ChangeActor) -> &'static str {
  match actor {
    ChangeActor::Person => "person",
    ChangeActor::Orchester => "orchester",
  }
}

fn parse_actor(text: &str) -> Result<ChangeActor, StoreError> {
  match text {
    "person" => Ok(ChangeActor::Person),
    "orchester" => Ok(ChangeActor::Orchester),
    other => Err(corrupt("module change actor", other)),
  }
}

struct ModuleRow {
  id: String,
  name: String,
  description: String,
  start: Option<String>,
  target: Option<String>,
  state: String,
  created_at: i64,
}

impl ModuleRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      name: row.get(1)?,
      description: row.get(2)?,
      start: row.get(3)?,
      target: row.get(4)?,
      state: row.get(5)?,
      created_at: row.get(6)?,
    })
  }

  fn into_module(self) -> Result<Module, StoreError> {
    let state = match self.state.as_str() {
      "open" => ModuleState::Open,
      "closed" => ModuleState::Closed,
      other => return Err(corrupt("module state", other)),
    };
    let start = self
      .start
      .as_deref()
      .map(|day| parse("module start date", day))
      .transpose()?;
    let target = self
      .target
      .as_deref()
      .map(|day| parse("module target date", day))
      .transpose()?;
    let details = ModuleDetails::new(
      Name::new(&self.name).map_err(|error| corrupt("module name", error))?,
      &self.description,
      (start, target),
      state,
    )
    .map_err(|error| corrupt("module dates", error))?;
    Ok(Module::new(
      parse("module id", &self.id)?,
      details,
      Timestamp::from_unix_millis(self.created_at),
    ))
  }
}
