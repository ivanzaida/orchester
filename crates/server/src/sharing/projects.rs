//! Server projects by name and slug (`docs/architecture/server.md`, "Servers and project addresses"): a project's
//! address is the server's address and its slug, fixed once created; the repository remote is only a hint.

use orchester_api::{ErrorCode, NewProject, ProjectChanges, ProjectCreated, ProjectLink, ProjectLinked, SharedProject};
use orchester_core::{
  id::{ProjectId, UserId},
  name::Name,
  project::ProjectKey,
  repository::RepositoryIdentity,
  resource::Slug,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{access, batches, decode, encode, events, members};
use crate::{db, error::ServerError};

/// Slugs no project can have: the server's own paths (row 62). Core's `Slug` already rules out names starting with
/// `_` or `.`.
pub(crate) const RESERVED_SLUGS: [&str; 3] = ["admin", "api", "auth"];

const COLUMNS: &str = "slug,key,name,remote_hint,next_task_number";

pub fn load(db: &Connection, project: ProjectId, user: UserId, base: &str) -> Result<SharedProject, ServerError> {
  let role = access::role(db, project, user)?;
  let row = db
    .query_row(
      &format!("SELECT {COLUMNS} FROM projects WHERE id=?1"),
      [project.to_string()],
      |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, Option<String>>(3)?,
          row.get::<_, i64>(4)?,
        ))
      },
    )
    .optional()?;
  let (slug, key, name, remote_hint, next_number) = row.ok_or(ErrorCode::NotFound)?;
  Ok(SharedProject {
    id: project,
    address: format!("{base}/{slug}"),
    slug,
    key,
    name,
    remote_hint: remote_hint
      .map(|remote| RepositoryIdentity::parse(&remote))
      .transpose()
      .map_err(|_| ServerError::StoredData)?,
    role,
    next_number: super::unsigned(next_number)?,
  })
}

/// The projects `user` is a member of.
pub fn list(db: &Connection, user: UserId, base: &str) -> Result<Vec<SharedProject>, ServerError> {
  let mut query = db.prepare("SELECT project FROM memberships WHERE user_id=?1 ORDER BY added_at,project")?;
  let ids = query
    .query_map([user.to_string()], |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  ids
    .into_iter()
    .map(|id| load(db, id.parse().map_err(|_| ServerError::StoredData)?, user, base))
    .collect()
}

/// The project at `slug` when `user` is a member; `404` both when there is none and when they are not a member.
pub fn by_slug(db: &Connection, user: UserId, slug: &str, base: &str) -> Result<SharedProject, ServerError> {
  let found: Option<String> = db
    .query_row("SELECT id FROM projects WHERE slug=?1", [slug], |row| row.get(0))
    .optional()?;
  let id = found
    .ok_or(ErrorCode::NotFound)?
    .parse()
    .map_err(|_| ServerError::StoredData)?;
  // A project the person is not a member of answers like no project: its name and owners are not theirs to read.
  if access::role(db, id, user)?.is_none() {
    return Err(ErrorCode::NotFound.into());
  }
  load(db, id, user, base)
}

pub fn create(
  db: &Connection,
  user: UserId,
  input: NewProject,
  base: &str,
  now: i64,
) -> Result<ProjectCreated, ServerError> {
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
  let slug = Slug::new(&input.slug).map_err(|_| ErrorCode::Invalid)?;
  let name = Name::new(&input.name).map_err(|_| ErrorCode::Invalid)?;
  let key = match &input.key {
    Some(key) => ProjectKey::new(key).map_err(|_| ErrorCode::Invalid)?,
    None => ProjectKey::suggest(&name),
  };
  let remote_hint = hint(input.remote_hint.as_deref())?;
  let taken: bool = db.query_row(
    "SELECT EXISTS(SELECT 1 FROM projects WHERE slug=?1)",
    [slug.as_str()],
    |row| row.get(0),
  )?;
  if taken || RESERVED_SLUGS.contains(&slug.as_str()) {
    return Err(ErrorCode::SlugTaken.into());
  }
  let project = ProjectId::generate();
  db.execute(
    "INSERT INTO projects(id,key,name,slug,remote_hint,next_task_number,created_by,created_at) VALUES (?1,?2,?3,?4,?5,1,?6,?7)",
    params![
      project.to_string(),
      key.as_str(),
      name.as_str(),
      slug.as_str(),
      remote_hint.as_ref().map(RepositoryIdentity::as_str),
      user.to_string(),
      now
    ],
  )?;
  db::audit(db, Some(user), "project_created", &project.to_string(), None, now)?;
  members::insert(db, project, user, orchester_api::MemberRole::Owner, user, now)?;
  let tasks = batches::upload(db, project, user, (input.tasks, Vec::new()), true, now)?;
  let response = ProjectCreated {
    project: load(db, project, user, base)?,
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

/// A checkout linked to `project` uploads its tasks: the first into a project with no tasks keeps their numbers, later
/// ones are imported with new numbers. The checkout's remote becomes the hint while the project has none.
pub fn link(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  input: ProjectLink,
  now: i64,
) -> Result<ProjectLinked, ServerError> {
  if let Some(response) = batches::repeated(db, project, input.idempotency_key)? {
    return linked(db, project, response);
  }
  let remote = hint(input.remote.as_deref())?;
  let empty: bool = db.query_row(
    "SELECT NOT EXISTS(SELECT 1 FROM tasks WHERE project=?1)",
    [project.to_string()],
    |row| row.get(0),
  )?;
  let response = batches::upload(db, project, user, (input.tasks, input.modules), empty, now)?;
  batches::remember(db, project, input.idempotency_key, user, &response)?;
  if let Some(remote) = remote {
    db.execute(
      "UPDATE projects SET remote_hint=?1 WHERE id=?2 AND remote_hint IS NULL",
      params![remote.as_str(), project.to_string()],
    )?;
  }
  linked(db, project, response)
}

fn linked(
  db: &Connection,
  project: ProjectId,
  response: orchester_api::TasksCreated,
) -> Result<ProjectLinked, ServerError> {
  Ok(ProjectLinked {
    tasks: response.tasks,
    dropped_links: response.dropped_links,
    cursor: events::cursor(db, project)?,
    modules: response.modules,
    mapped: response.mapped,
  })
}

/// A repository identity from an untrusted remote or fetch URL, credentials dropped.
fn hint(remote: Option<&str>) -> Result<Option<RepositoryIdentity>, ServerError> {
  remote
    .map(|remote| RepositoryIdentity::parse(remote).map_err(|_| ErrorCode::Invalid.into()))
    .transpose()
}

pub fn patch(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  input: ProjectChanges,
  base: &str,
  now: i64,
) -> Result<SharedProject, ServerError> {
  let mut value = load(db, project, user, base)?;
  let original = (value.name.clone(), value.key.clone(), value.remote_hint.clone());
  if let Some(name) = input.name {
    value.name = Name::new(&name).map_err(|_| ErrorCode::Invalid)?.as_str().into();
  }
  if let Some(key) = input.key {
    value.key = ProjectKey::new(&key).map_err(|_| ErrorCode::Invalid)?.as_str().into();
  }
  if let Some(remote) = input.remote_hint {
    value.remote_hint = if remote.trim().is_empty() {
      None
    } else {
      hint(Some(&remote))?
    };
  }
  if original == (value.name.clone(), value.key.clone(), value.remote_hint.clone()) {
    return Ok(value);
  }
  db.execute(
    "UPDATE projects SET name=?1,key=?2,remote_hint=?3 WHERE id=?4",
    params![
      value.name,
      value.key,
      value.remote_hint.as_ref().map(RepositoryIdentity::as_str),
      project.to_string()
    ],
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
