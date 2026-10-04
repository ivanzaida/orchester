use orchester_api::{ErrorCode, SharedTag, TagsChanged};
use orchester_core::{
  id::{ProjectId, TagId, UserId},
  name::Name,
  task::organization::{ChangeActor, OrganizationError, Tag},
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{events, organization, task_rows, task_write};
use crate::error::ServerError;

pub fn list(db: &Connection, project: ProjectId) -> Result<Vec<SharedTag>, ServerError> {
  let mut query = db.prepare("SELECT id,name FROM tags WHERE project=?1 ORDER BY normalized,id")?;
  let rows = query
    .query_map([project.to_string()], |row| {
      Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(id, name)| {
      Ok(SharedTag {
        id: id.parse().map_err(|_| ServerError::StoredData)?,
        name,
      })
    })
    .collect()
}

fn find(db: &Connection, project: ProjectId, name: &Name) -> Result<Option<SharedTag>, ServerError> {
  let result: Option<(String, String)> = db
    .query_row(
      "SELECT id,name FROM tags WHERE project=?1 AND normalized=?2",
      params![project.to_string(), name.as_str().to_lowercase()],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?;
  result
    .map(|(id, name)| {
      Ok(SharedTag {
        id: id.parse().map_err(|_| ServerError::StoredData)?,
        name,
      })
    })
    .transpose()
}

fn insert(db: &Connection, project: ProjectId, name: Name) -> Result<SharedTag, ServerError> {
  let tag = Tag::new(TagId::generate(), name);
  db.execute(
    "INSERT INTO tags VALUES (?1,?2,?3,?4)",
    params![
      tag.id().to_string(),
      project.to_string(),
      tag.name().as_str(),
      tag.name().as_str().to_lowercase()
    ],
  )?;
  Ok(SharedTag {
    id: tag.id(),
    name: tag.name().as_str().into(),
  })
}

pub fn upload_names(db: &Connection, project: ProjectId, names: Vec<String>) -> Result<Vec<TagId>, ServerError> {
  let mut tags = Vec::new();
  for name in names {
    let name = Name::new(&name).map_err(|_| ErrorCode::Invalid)?;
    let tag = match find(db, project, &name)? {
      Some(tag) => tag,
      None => insert(db, project, name)?,
    };
    tags.push(tag.id);
  }
  Ok(tags)
}

pub fn create(db: &Connection, project: ProjectId, name: &str, now: i64) -> Result<SharedTag, ServerError> {
  let name = Name::new(name).map_err(|_| ErrorCode::Invalid)?;
  if find(db, project, &name)?.is_some() {
    return Err(organization::failure(&OrganizationError::DuplicateTag));
  }
  let tag = insert(db, project, name)?;
  event(db, project, now)?;
  Ok(tag)
}

pub fn rename(db: &Connection, project: ProjectId, id: TagId, name: &str, now: i64) -> Result<SharedTag, ServerError> {
  let old = list(db, project)?
    .into_iter()
    .find(|tag| tag.id == id)
    .ok_or(ErrorCode::NotFound)?;
  let name = Name::new(name).map_err(|_| ErrorCode::Invalid)?;
  if find(db, project, &name)?.is_some_and(|tag| tag.id != id) {
    return Err(organization::failure(&OrganizationError::DuplicateTag));
  }
  let tag = Tag::new(id, name);
  if old.name == tag.name().as_str() {
    return Ok(old);
  }
  db.execute(
    "UPDATE tags SET name=?1,normalized=?2 WHERE id=?3",
    params![tag.name().as_str(), tag.name().as_str().to_lowercase(), id.to_string()],
  )?;
  event(db, project, now)?;
  Ok(SharedTag {
    id,
    name: tag.name().as_str().into(),
  })
}

pub fn remove(
  db: &Connection,
  project: ProjectId,
  id: TagId,
  into: Option<TagId>,
  user: UserId,
  now: i64,
) -> Result<TagsChanged, ServerError> {
  let tags = list(db, project)?;
  let source = tags.iter().find(|tag| tag.id == id).ok_or(ErrorCode::NotFound)?;
  let source = Tag::new(source.id, Name::new(&source.name).map_err(|_| ServerError::StoredData)?);
  if let Some(into) = into {
    source
      .check_merge(into)
      .map_err(|error| organization::failure(&error))?;
    if !tags.iter().any(|tag| tag.id == into) {
      return Err(ErrorCode::NotFound.into());
    }
  }
  let mut changed = Vec::new();
  for task in task_rows::ids(db, project)? {
    let from = organization::load(db, task)?;
    if !from.tags().contains(&id) {
      continue;
    }
    let mut to = from.clone().remove_tags(&[id]);
    if let Some(into) = into {
      to = to.add_tags(&[into]);
    }
    organization::record(db, task, user, ChangeActor::Person, now, (from, to.clone()))?;
    organization::save(db, task, &to)?;
    task_write::bump(db, task, user, now)?;
    events::task(db, project, task, now)?;
    changed.push(task_rows::load(db, project, task, now)?);
  }
  db.execute("DELETE FROM tags WHERE id=?1", [id.to_string()])?;
  event(db, project, now)?;
  Ok(TagsChanged {
    tags: list(db, project)?,
    tasks: changed,
  })
}

pub fn event(db: &Connection, project: ProjectId, now: i64) -> Result<(), ServerError> {
  events::append(
    db,
    project,
    orchester_api::EventKind::Tags,
    &orchester_api::Tags {
      tags: list(db, project)?,
    },
    now,
  )
}
