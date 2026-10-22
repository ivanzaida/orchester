//! A shared project's modules (`docs/architecture/server.md`, "Modules"): core's module rules
//! (`orchester_core::module`) over the project's modules, each under a revision; names are unique in the project
//! without regard to case. Which module a task is in is a field of the task, and every move is kept (`module_changes`)
//! for each desktop's burndown. A module create or change is answered once per idempotency key.

use std::collections::HashMap;

use orchester_api::{
  Attribution, ErrorCode, EventKind, FieldsError, MappedModule, ModuleCreate, ModuleFields, ModulePatch,
  ModulesChanged, SharedModule, SharedModuleChange, SharingErrorDetails, UploadModule,
};
use orchester_core::{
  id::{IdempotencyKey, ModuleId, ProjectId, TaskId, UserId},
  module::{ModuleDetails, ModuleError, ModuleState},
  sharing::TaskRevision,
  task::organization::ChangeActor,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{access, conflict, decode, encode, events, reason, sql_integer, unsigned};
use crate::error::ServerError;

const COLUMNS: &str =
  "id,name,description,start_date,target_date,state,revision,created_by,created_at,updated_by,updated_at";

pub fn list(db: &Connection, project: ProjectId) -> Result<Vec<SharedModule>, ServerError> {
  let mut query = db.prepare(&format!(
    "SELECT {COLUMNS} FROM modules WHERE project=?1 ORDER BY created_at,id"
  ))?;
  let mut rows = query.query([project.to_string()])?;
  let mut modules = Vec::new();
  while let Some(row) = rows.next()? {
    modules.push(read(db, row)?);
  }
  Ok(modules)
}

pub fn load(db: &Connection, project: ProjectId, id: ModuleId) -> Result<SharedModule, ServerError> {
  find(db, project, "id=?2", &id.to_string())?.ok_or_else(|| ErrorCode::NotFound.into())
}

fn find(db: &Connection, project: ProjectId, filter: &str, value: &str) -> Result<Option<SharedModule>, ServerError> {
  let mut query = db.prepare(&format!("SELECT {COLUMNS} FROM modules WHERE project=?1 AND {filter}"))?;
  let mut rows = query.query(params![project.to_string(), value])?;
  rows.next()?.map(|row| read(db, row)).transpose()
}

/// The project's module named `details`' name without regard to case.
fn named(db: &Connection, project: ProjectId, details: &ModuleDetails) -> Result<Option<SharedModule>, ServerError> {
  find(db, project, "normalized=?2", &normalized(details))
}

fn normalized(details: &ModuleDetails) -> String {
  details.name().as_str().to_lowercase()
}

fn read(db: &Connection, row: &Row<'_>) -> Result<SharedModule, ServerError> {
  let id: String = row.get(0)?;
  let state = match row.get::<_, String>(5)?.as_str() {
    "open" => ModuleState::Open,
    "closed" => ModuleState::Closed,
    _ => return Err(ServerError::StoredData),
  };
  let created: String = row.get(7)?;
  let updated: String = row.get(9)?;
  Ok(SharedModule {
    id: id.parse().map_err(|_| ServerError::StoredData)?,
    fields: ModuleFields {
      name: row.get(1)?,
      description: row.get(2)?,
      start: row.get(3)?,
      target: row.get(4)?,
      state,
    },
    revision: TaskRevision::try_from(unsigned(row.get(6)?)?).map_err(|_| ServerError::StoredData)?,
    created: Attribution {
      by: access::user_ref(db, created.parse().map_err(|_| ServerError::StoredData)?)?,
      at: row.get(8)?,
    },
    updated: Attribution {
      by: access::user_ref(db, updated.parse().map_err(|_| ServerError::StoredData)?)?,
      at: row.get(10)?,
    },
  })
}

const fn state_name(state: ModuleState) -> &'static str {
  match state {
    ModuleState::Open => "open",
    ModuleState::Closed => "closed",
  }
}

/// `fields` as core's module details; a malformed name or day is `invalid`, a target before the start `module`.
fn details(fields: &ModuleFields) -> Result<ModuleDetails, ServerError> {
  fields.details().map_err(|error| refused(&error))
}

fn refused(error: &FieldsError) -> ServerError {
  match error {
    FieldsError::Invalid => ErrorCode::Invalid.into(),
    FieldsError::Module(ModuleError::TargetBeforeStart) => reason(ErrorCode::Module, "target_before_start"),
    // Fields alone are never a duplicate, missing or stale module.
    FieldsError::Module(_) => ServerError::StoredData,
  }
}

fn exists(module: SharedModule) -> ServerError {
  conflict(
    ErrorCode::ModuleExists,
    SharingErrorDetails::Module {
      module: Box::new(module),
    },
  )
}

/// A task names a module the project does not have.
fn not_found() -> ServerError {
  reason(ErrorCode::Module, "module_not_found")
}

fn insert(
  db: &Connection,
  project: ProjectId,
  (id, details): (ModuleId, &ModuleDetails),
  user: UserId,
  (created_at, now): (i64, i64),
) -> Result<(), ServerError> {
  db.execute(
    "INSERT INTO modules(id,project,name,normalized,description,start_date,target_date,state,revision,created_by,created_at,updated_by,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?10,?9,?11)",
    params![
      id.to_string(),
      project.to_string(),
      details.name().as_str(),
      normalized(details),
      details.description(),
      details.start().map(|day| day.to_string()),
      details.target().map(|day| day.to_string()),
      state_name(details.state()),
      user.to_string(),
      created_at,
      now
    ],
  )?;
  Ok(())
}

/// The project the module `id` is in, on this server.
fn project_of(db: &Connection, id: ModuleId) -> Result<Option<ProjectId>, ServerError> {
  let project: Option<String> = db
    .query_row("SELECT project FROM modules WHERE id=?1", [id.to_string()], |row| {
      row.get(0)
    })
    .optional()?;
  project
    .map(|id| id.parse().map_err(|_| ServerError::StoredData))
    .transpose()
}

pub fn create(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  input: &ModuleCreate,
  now: i64,
) -> Result<SharedModule, ServerError> {
  if let Some(module) = repeated(db, project, user, input.idempotency_key)? {
    return Ok(module);
  }
  let details = details(&input.module)?;
  if let Some(existing) = named(db, project, &details)? {
    return Err(exists(existing));
  }
  match project_of(db, input.id)? {
    Some(other) if other == project => return Err(ErrorCode::IdTaken.into()),
    // As for tasks: an id taken in another project is a conflict that names nothing of it.
    Some(_) => return Err(ErrorCode::Conflict.into()),
    None => {}
  }
  insert(db, project, (input.id, &details), user, (now, now))?;
  event(db, project, now)?;
  let module = load(db, project, input.id)?;
  remember(db, project, user, input.idempotency_key, &module, now)?;
  Ok(module)
}

/// Replaces the fields of module `id`, read at `input.revision`; a newer module is a `revision_conflict` with it.
pub fn patch(
  db: &Connection,
  project: ProjectId,
  id: ModuleId,
  user: UserId,
  input: &ModulePatch,
  now: i64,
) -> Result<SharedModule, ServerError> {
  if let Some(module) = repeated(db, project, user, input.idempotency_key)? {
    return Ok(module);
  }
  let old = load(db, project, id)?;
  if old.revision != input.revision {
    return Err(conflict(
      ErrorCode::RevisionConflict,
      SharingErrorDetails::ModuleRevision { current: Box::new(old) },
    ));
  }
  let details = details(&input.module)?;
  if let Some(existing) = named(db, project, &details)?.filter(|existing| existing.id != id) {
    return Err(exists(existing));
  }
  let module = if ModuleFields::from_core(&details) == old.fields {
    old
  } else {
    update(db, (project, id), &details, (old.revision, user), now)?;
    event(db, project, now)?;
    load(db, project, id)?
  };
  remember(db, project, user, input.idempotency_key, &module, now)?;
  Ok(module)
}

fn update(
  db: &Connection,
  (project, id): (ProjectId, ModuleId),
  details: &ModuleDetails,
  (revision, user): (TaskRevision, UserId),
  now: i64,
) -> Result<(), ServerError> {
  let next = revision.next().map_err(|_| ErrorCode::Invalid)?;
  db.execute(
    "UPDATE modules SET name=?3,normalized=?4,description=?5,start_date=?6,target_date=?7,state=?8,revision=?9,updated_by=?10,updated_at=?11 WHERE project=?1 AND id=?2",
    params![
      project.to_string(),
      id.to_string(),
      details.name().as_str(),
      normalized(details),
      details.description(),
      details.start().map(|day| day.to_string()),
      details.target().map(|day| day.to_string()),
      state_name(details.state()),
      sql_integer(next.get())?,
      user.to_string(),
      now
    ],
  )?;
  Ok(())
}

/// The first answer to `key` of `user` in `project`, kept as long as events are.
fn repeated(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  key: IdempotencyKey,
) -> Result<Option<SharedModule>, ServerError> {
  let response: Option<String> = db
    .query_row(
      "SELECT response FROM change_keys WHERE project=?1 AND user_id=?2 AND key=?3",
      params![project.to_string(), user.to_string(), key.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  response.map(|response| decode(&response)).transpose()
}

fn remember(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  key: IdempotencyKey,
  module: &SharedModule,
  now: i64,
) -> Result<(), ServerError> {
  db.execute(
    "INSERT INTO change_keys(project,user_id,key,response,at) VALUES (?1,?2,?3,?4,?5)",
    params![
      project.to_string(),
      user.to_string(),
      key.to_string(),
      encode(module)?,
      now
    ],
  )?;
  Ok(())
}

/// Takes the uploaded `modules` into the project: each becomes the project's module of its name, or a new one with
/// its id. What each uploaded id is in the project now.
pub fn upload(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  modules: Vec<UploadModule>,
  now: i64,
) -> Result<HashMap<ModuleId, ModuleId>, ServerError> {
  let mut ids = HashMap::new();
  let mut added = false;
  for upload in modules {
    let details = details(&upload.module)?;
    let id = match (named(db, project, &details)?, project_of(db, upload.id)?) {
      (Some(existing), _) => existing.id,
      // An upload repeated after the module was renamed here.
      (None, Some(other)) if other == project => upload.id,
      (None, Some(_)) => return Err(ErrorCode::Conflict.into()),
      (None, None) => {
        insert(db, project, (upload.id, &details), user, (upload.created_at, now))?;
        added = true;
        upload.id
      }
    };
    if ids.insert(upload.id, id).is_some() {
      return Err(ErrorCode::Invalid.into());
    }
  }
  if added {
    event(db, project, now)?;
  }
  Ok(ids)
}

/// The uploaded modules that became another module of the project.
pub fn mapped(ids: &HashMap<ModuleId, ModuleId>) -> Vec<MappedModule> {
  let mut mapped: Vec<_> = ids
    .iter()
    .filter(|(from, to)| from != to)
    .map(|(from, to)| MappedModule { from: *from, to: *to })
    .collect();
  mapped.sort_by_key(|mapped| mapped.from);
  mapped
}

/// The module of the project an uploaded task's `module` is now: one of the upload's (`ids`) or one the project has.
pub fn resolve(
  db: &Connection,
  project: ProjectId,
  ids: &HashMap<ModuleId, ModuleId>,
  module: Option<ModuleId>,
) -> Result<Option<ModuleId>, ServerError> {
  let Some(module) = module else {
    return Ok(None);
  };
  if let Some(id) = ids.get(&module) {
    return Ok(Some(*id));
  }
  check(db, project, Some(module))?;
  Ok(Some(module))
}

/// Refuses a module the project does not have.
pub fn check(db: &Connection, project: ProjectId, module: Option<ModuleId>) -> Result<(), ServerError> {
  match module {
    Some(module) if project_of(db, module)? != Some(project) => Err(not_found()),
    _ => Ok(()),
  }
}

/// Keeps the move of `task` from one module to another, by `user` as `actor`.
pub fn record(
  db: &Connection,
  project: ProjectId,
  task: TaskId,
  (from, to): (Option<ModuleId>, Option<ModuleId>),
  (user, actor): (UserId, ChangeActor),
  now: i64,
) -> Result<(), ServerError> {
  db.execute(
    "INSERT INTO module_changes(project,task,from_module,to_module,user_id,actor,at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
    params![
      project.to_string(),
      task.to_string(),
      from.map(|id| id.to_string()),
      to.map(|id| id.to_string()),
      user.to_string(),
      actor_name(actor),
      now
    ],
  )?;
  Ok(())
}

const fn actor_name(actor: ChangeActor) -> &'static str {
  match actor {
    ChangeActor::Person => "person",
    ChangeActor::Orchester => "orchester",
  }
}

/// The moves of the project's tasks after the sequence `after`, oldest first.
pub fn changes(db: &Connection, project: ProjectId, after: u64) -> Result<Vec<SharedModuleChange>, ServerError> {
  let mut query = db.prepare(
    "SELECT sequence,task,from_module,to_module,user_id,actor,at FROM module_changes WHERE project=?1 AND sequence>?2 ORDER BY sequence",
  )?;
  let mut rows = query.query(params![project.to_string(), sql_integer(after)?])?;
  let mut changes = Vec::new();
  while let Some(row) = rows.next()? {
    let id = |index: usize| -> Result<Option<ModuleId>, ServerError> {
      row
        .get::<_, Option<String>>(index)?
        .map(|id| id.parse().map_err(|_| ServerError::StoredData))
        .transpose()
    };
    let task: String = row.get(1)?;
    let user: String = row.get(4)?;
    changes.push(SharedModuleChange {
      sequence: unsigned(row.get(0)?)?,
      task: task.parse().map_err(|_| ServerError::StoredData)?,
      from: id(2)?,
      to: id(3)?,
      by: access::user_ref(db, user.parse().map_err(|_| ServerError::StoredData)?)?,
      actor: match row.get::<_, String>(5)?.as_str() {
        "person" => ChangeActor::Person,
        "orchester" => ChangeActor::Orchester,
        _ => return Err(ServerError::StoredData),
      },
      at: row.get(6)?,
    });
  }
  Ok(changes)
}

pub fn event(db: &Connection, project: ProjectId, now: i64) -> Result<(), ServerError> {
  events::append(
    db,
    project,
    EventKind::Modules,
    &ModulesChanged {
      modules: list(db, project)?,
    },
    now,
  )
}
