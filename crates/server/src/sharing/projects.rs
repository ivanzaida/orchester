use orchester_api::{
  ErrorCode, MemberRole, ProjectChanges, ProjectCreate, ProjectCreated, SharedProject, SharingErrorDetails,
};
use orchester_core::{
  id::{ProjectId, UserId},
  name::Name,
  project::ProjectKey,
  repository::RepositoryIdentity,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{access, batches, conflict, decode, encode, events, members};
use crate::{db, error::ServerError};

pub fn load(db: &Connection, project: ProjectId, user: UserId) -> Result<SharedProject, ServerError> {
  let role = access::role(db, project, user)?.unwrap_or(MemberRole::Owner);
  Ok(
    db.query_row(
      "SELECT key,name,remote,next_task_number FROM projects WHERE id=?1",
      [project.to_string()],
      |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, i64>(3)?,
        ))
      },
    )
    .optional()?
    .map(|(key, name, remote, next_number)| {
      Ok::<_, ServerError>(SharedProject {
        id: project,
        key,
        name,
        remote: RepositoryIdentity::parse(&remote).map_err(|_| ServerError::StoredData)?,
        role,
        next_number: super::unsigned(next_number)?,
      })
    })
    .transpose()?
    .ok_or(ErrorCode::NotFound)?,
  )
}

pub fn lookup(
  db: &Connection,
  user: UserId,
  remote: Option<RepositoryIdentity>,
) -> Result<Vec<SharedProject>, ServerError> {
  if let Some(remote) = remote {
    let id: Option<String> = db
      .query_row("SELECT id FROM projects WHERE remote=?1", [remote.as_str()], |row| {
        row.get(0)
      })
      .optional()?;
    let Some(id) = id else {
      return Ok(Vec::new());
    };
    let id = id.parse().map_err(|_| ServerError::StoredData)?;
    if access::role(db, id, user)?.is_none() {
      return Err(conflict(
        ErrorCode::NotMember,
        SharingErrorDetails::Owners {
          owners: access::owners(db, id)?,
        },
      ));
    }
    return Ok(vec![load(db, id, user)?]);
  }
  let mut query = db.prepare("SELECT project FROM memberships WHERE user_id=?1 ORDER BY added_at,project")?;
  let ids = query
    .query_map([user.to_string()], |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  ids
    .into_iter()
    .map(|id| load(db, id.parse().map_err(|_| ServerError::StoredData)?, user))
    .collect()
}

pub fn create(db: &Connection, user: UserId, input: ProjectCreate, now: i64) -> Result<ProjectCreated, ServerError> {
  let repeated: Option<String> = db
    .query_row(
      "SELECT response FROM project_batches WHERE user_id=?1 AND key=?2",
      params![user.to_string(), input.idempotency_key.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  if let Some(repeated) = repeated {
    let response: ProjectCreated = decode(&repeated)?;
    if access::role(db, response.project.id, user)?.is_none() {
      return Err(ErrorCode::NotFound.into());
    }
    return Ok(response);
  }
  let remote = RepositoryIdentity::parse(&input.remote).map_err(|_| ErrorCode::Invalid)?;
  let existing: Option<String> = db
    .query_row("SELECT id FROM projects WHERE remote=?1", [remote.as_str()], |row| {
      row.get(0)
    })
    .optional()?;
  if let Some(id) = existing {
    let project = id.parse().map_err(|_| ServerError::StoredData)?;
    return Err(conflict(
      ErrorCode::RemoteTaken,
      SharingErrorDetails::Remote {
        project,
        member: access::role(db, project, user)?.is_some(),
        owners: access::owners(db, project)?,
      },
    ));
  }
  let name = Name::new(&input.name).map_err(|_| ErrorCode::Invalid)?;
  let key = ProjectKey::new(&input.key).map_err(|_| ErrorCode::Invalid)?;
  let project = ProjectId::generate();
  db.execute(
    "INSERT INTO projects(id,key,name,remote,next_task_number,created_by,created_at) VALUES (?1,?2,?3,?4,1,?5,?6)",
    params![
      project.to_string(),
      key.as_str(),
      name.as_str(),
      remote.as_str(),
      user.to_string(),
      now
    ],
  )?;
  db.execute(
    "INSERT INTO memberships VALUES (?1,?2,'owner',?2,?3,?4)",
    params![
      project.to_string(),
      user.to_string(),
      now,
      orchester_core::id::IdempotencyKey::generate().to_string()
    ],
  )?;
  db::audit(db, Some(user), "member_added", &format!("{project}/{user}"), None, now)?;
  members::event(db, project, now)?;
  let tasks = batches::upload(db, project, user, input.tasks, true, now)?;
  let response = ProjectCreated {
    project: load(db, project, user)?,
    tasks: tasks.tasks,
    cursor: events::cursor(db, project)?,
    dropped_links: tasks.dropped_links,
  };
  db.execute(
    "INSERT INTO project_batches VALUES (?1,?2,?3)",
    params![user.to_string(), input.idempotency_key.to_string(), encode(&response)?],
  )?;
  Ok(response)
}

pub fn patch(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  input: ProjectChanges,
  now: i64,
) -> Result<SharedProject, ServerError> {
  let mut value = load(db, project, user)?;
  let original = (value.name.clone(), value.key.clone());
  if let Some(name) = input.name {
    value.name = Name::new(&name).map_err(|_| ErrorCode::Invalid)?.as_str().into();
  }
  if let Some(key) = input.key {
    value.key = ProjectKey::new(&key).map_err(|_| ErrorCode::Invalid)?.as_str().into();
  }
  if original == (value.name.clone(), value.key.clone()) {
    return Ok(value);
  }
  db.execute(
    "UPDATE projects SET name=?1,key=?2 WHERE id=?3",
    params![value.name, value.key, project.to_string()],
  )?;
  events::append(
    db,
    project,
    orchester_api::EventKind::Project,
    &orchester_api::ProjectUpdated {
      id: project,
      key: value.key.clone(),
      name: value.name.clone(),
    },
    now,
  )?;
  Ok(value)
}
