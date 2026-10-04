use orchester_core::{
  id::{TagId, TaskId},
  name::Name,
  task::organization::{ChangeActor, OrganizationError, Tag},
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{ProjectDb, organization};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

impl ProjectDb {
  pub fn project_tags(&self) -> Result<Vec<Tag>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT id, name FROM project_tags ORDER BY normalized")?;
    let rows = statement
      .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(id, name)| {
        Ok(Tag::new(
          parse("tag id", &id)?,
          Name::new(&name).map_err(|error| corrupt("tag name", error))?,
        ))
      })
      .collect()
  }

  pub fn create_project_tag(&mut self, name: Name) -> Result<Tag, StoreError> {
    let transaction = self.connection.transaction()?;
    super::sharing::ensure_local_project(&transaction)?;
    unique_name(&transaction, &name, None)?;
    let tag = Tag::new(TagId::generate(), name);
    transaction.execute(
      "INSERT INTO project_tags(id, name, normalized) VALUES (?1, ?2, ?3)",
      params![
        tag.id().to_string(),
        tag.name().as_str(),
        tag.name().as_str().to_lowercase()
      ],
    )?;
    transaction.commit()?;
    Ok(tag)
  }

  pub fn rename_project_tag(&mut self, id: TagId, name: &Name) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    super::sharing::ensure_local_project(&transaction)?;
    ensure_tag(&transaction, id)?;
    unique_name(&transaction, name, Some(id))?;
    transaction.execute(
      "UPDATE project_tags SET name = ?2, normalized = ?3 WHERE id = ?1",
      params![id.to_string(), name.as_str(), name.as_str().to_lowercase()],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// Caller confirms the removal of every assignment before invoking this operation.
  pub fn delete_project_tag(&mut self, id: TagId, at: Timestamp) -> Result<(), StoreError> {
    self.replace_project_tag(id, None, at)
  }

  /// Union assignments into the existing target, then remove the source atomically.
  pub fn merge_project_tags(&mut self, source: TagId, target: TagId, at: Timestamp) -> Result<(), StoreError> {
    if source == target {
      return Err(OrganizationError::SameTag.into());
    }
    self.replace_project_tag(source, Some(target), at)
  }

  /// How many tasks carry each tag, read in one query; a tag on no task is left out.
  pub fn tag_task_counts(&self) -> Result<std::collections::HashMap<TagId, usize>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT tag.value, COUNT(DISTINCT organization.task_id)
       FROM task_organization AS organization, json_each(organization.body, '$.body.tags') AS tag
       GROUP BY tag.value",
    )?;
    let rows = statement
      .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(id, count)| {
        let id = crate::rows::parse("tag id", &id)?;
        let count = usize::try_from(count).map_err(|_| crate::rows::corrupt("tag count", "negative count"))?;
        Ok((id, count))
      })
      .collect()
  }

  fn replace_project_tag(&mut self, source: TagId, target: Option<TagId>, at: Timestamp) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    super::sharing::ensure_local_project(&transaction)?;
    ensure_tag(&transaction, source)?;
    if let Some(target) = target {
      ensure_tag(&transaction, target)?;
    }
    for task in all_tasks(&transaction)? {
      let value = organization::load(&transaction, task)?;
      if !value.tags().contains(&source) {
        continue;
      }
      let mut tags: Vec<_> = value.tags().iter().copied().filter(|id| *id != source).collect();
      tags.extend(target);
      organization::write(&transaction, task, &value.with_tags(tags), ChangeActor::Person, at)?;
    }
    transaction.execute("DELETE FROM project_tags WHERE id = ?1", [source.to_string()])?;
    transaction.commit()?;
    Ok(())
  }
}

pub(super) fn ensure_tag(connection: &Connection, id: TagId) -> Result<(), StoreError> {
  let exists: bool = connection.query_row(
    "SELECT EXISTS(SELECT 1 FROM project_tags WHERE id = ?1)",
    [id.to_string()],
    |row| row.get(0),
  )?;
  if !exists {
    return Err(OrganizationError::TagNotFound(id).into());
  }
  Ok(())
}

fn unique_name(connection: &Connection, name: &Name, except: Option<TagId>) -> Result<(), StoreError> {
  let existing: Option<String> = connection
    .query_row(
      "SELECT id FROM project_tags WHERE normalized = ?1",
      [name.as_str().to_lowercase()],
      |row| row.get(0),
    )
    .optional()?;
  if let Some(id) = existing
    && Some(parse("tag id", &id)?) != except
  {
    return Err(OrganizationError::DuplicateTag.into());
  }
  Ok(())
}

fn all_tasks(connection: &Connection) -> Result<Vec<TaskId>, StoreError> {
  let mut statement = connection.prepare("SELECT id FROM tasks ORDER BY number")?;
  let ids = statement
    .query_map([], |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  ids.into_iter().map(|id| parse("task id", &id)).collect()
}
