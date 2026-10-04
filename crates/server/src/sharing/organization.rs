use std::collections::{HashMap, HashSet};

use orchester_api::{ErrorCode, Organization, OrganizationHistory, SharedOrganizationChange};
use orchester_core::{
  id::{ProjectId, TagId, TaskId, UserId},
  task::organization::{BlockingTask, ChangeActor, OrganizationChange, OrganizationError, TaskOrganization},
  time::Timestamp,
};
use rusqlite::{Connection, params};

use super::{access, decode, encode, reason, task_rows};
use crate::error::ServerError;

pub fn failure(error: &OrganizationError) -> ServerError {
  match error {
    OrganizationError::MissingReason => reason(ErrorCode::Organization, "missing_reason"),
    OrganizationError::DependencyCycle => reason(ErrorCode::Organization, "dependency_cycle"),
    OrganizationError::StopRunFirst(_) => reason(ErrorCode::Organization, "stop_run_first"),
    OrganizationError::TagNotFound(_) => reason(ErrorCode::Organization, "tag_not_found"),
    OrganizationError::DuplicateTag => ErrorCode::TagExists.into(),
    OrganizationError::SameTag => ErrorCode::SameTag.into(),
    // Run preflight belongs to the runner; no S1b organization write can produce this error.
    OrganizationError::Blocked(_) => ServerError::StoredData,
  }
}

pub fn load(db: &Connection, task: TaskId) -> Result<TaskOrganization, ServerError> {
  let (status, priority, blocked): (String, String, Option<String>) = db.query_row(
    "SELECT workflow,priority,blocked_reason FROM tasks WHERE id=?1",
    [task.to_string()],
    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
  )?;
  let mut query = db.prepare("SELECT tag FROM task_tags WHERE task=?1 ORDER BY tag")?;
  let tags = query
    .query_map([task.to_string()], |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  let tags = tags
    .into_iter()
    .map(|tag| tag.parse().map_err(|_| ServerError::StoredData))
    .collect::<Result<Vec<_>, _>>()?;
  let mut query = db.prepare("SELECT blocking,gate FROM task_links WHERE task=?1 ORDER BY blocking")?;
  let links = query
    .query_map([task.to_string()], |row| {
      Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  let links = links
    .into_iter()
    .map(|(task, gate)| {
      Ok(BlockingTask {
        task: task.parse().map_err(|_| ServerError::StoredData)?,
        gate: decode(&gate)?,
      })
    })
    .collect::<Result<Vec<_>, ServerError>>()?;
  TaskOrganization::new(decode(&status)?, decode(&priority)?, blocked, links, tags).map_err(|_| ServerError::StoredData)
}

pub fn validate(
  db: &Connection,
  project: ProjectId,
  task: TaskId,
  old: &TaskOrganization,
  value: &TaskOrganization,
) -> Result<(), ServerError> {
  validate_planned(db, project, task, old, value, &HashMap::new())
}

pub fn validate_planned(
  db: &Connection,
  project: ProjectId,
  task: TaskId,
  old: &TaskOrganization,
  value: &TaskOrganization,
  planned: &HashMap<TaskId, TaskOrganization>,
) -> Result<(), ServerError> {
  let known: HashSet<_> = super::tags::list(db, project)?.into_iter().map(|tag| tag.id).collect();
  value.check_tags(&known).map_err(|error| failure(&error))?;
  if old.status() != value.status()
    && !value
      .status()
      .can_change(task_rows::run(db, task)?.is_some_and(|run| run.phase.run_not_ended()))
  {
    return Err(reason(ErrorCode::Organization, "stop_run_first"));
  }
  let mut graph = HashMap::new();
  let mut pending: Vec<_> = value.dependencies().iter().map(|link| link.task).collect();
  while let Some(id) = pending.pop() {
    if graph.contains_key(&id) {
      continue;
    }
    if task_rows::project_of(db, id)? != Some(project) {
      return Err(ErrorCode::Invalid.into());
    }
    let other = match planned.get(&id) {
      Some(other) => other.clone(),
      None => load(db, id)?,
    };
    pending.extend(other.dependencies().iter().map(|link| link.task));
    graph.insert(id, other);
  }
  graph.insert(task, value.clone());
  value.check_dependencies(task, &graph).map_err(|error| failure(&error))
}

pub fn save(db: &Connection, task: TaskId, value: &TaskOrganization) -> Result<(), ServerError> {
  db.execute(
    "UPDATE tasks SET workflow=?1,priority=?2,blocked_reason=?3 WHERE id=?4",
    params![
      encode(&value.status())?,
      encode(&value.priority())?,
      value.blocked_reason(),
      task.to_string()
    ],
  )?;
  db.execute("DELETE FROM task_tags WHERE task=?1", [task.to_string()])?;
  for tag in value.tags() {
    db.execute(
      "INSERT INTO task_tags VALUES (?1,?2)",
      params![task.to_string(), tag.to_string()],
    )?;
  }
  db.execute("DELETE FROM task_links WHERE task=?1", [task.to_string()])?;
  for link in value.dependencies() {
    db.execute(
      "INSERT INTO task_links VALUES (?1,?2,?3)",
      params![task.to_string(), link.task.to_string(), encode(&link.gate)?],
    )?;
  }
  Ok(())
}

fn names(db: &Connection, tags: &[TagId]) -> Result<Vec<orchester_core::name::Name>, ServerError> {
  tags
    .iter()
    .map(|tag| {
      let name: String = db.query_row("SELECT name FROM tags WHERE id=?1", [tag.to_string()], |row| row.get(0))?;
      orchester_core::name::Name::new(&name).map_err(|_| ServerError::StoredData)
    })
    .collect()
}

pub fn record(
  db: &Connection,
  task: TaskId,
  by: UserId,
  actor: ChangeActor,
  now: i64,
  transition: (TaskOrganization, TaskOrganization),
) -> Result<(), ServerError> {
  let (from, to) = transition;
  let change = OrganizationChange {
    from_tag_names: names(db, from.tags())?,
    to_tag_names: names(db, to.tags())?,
    actor,
    at: Timestamp::from_unix_millis(now),
    from,
    to,
  };
  db.execute("INSERT INTO organization_changes(task,user_id,actor,at,before,after,from_tag_names,to_tag_names) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
    params![task.to_string(),by.to_string(),encode(&change.actor)?,change.at.unix_millis(),encode(&change.from)?,encode(&change.to)?,encode(&change.from_tag_names)?,encode(&change.to_tag_names)?])?;
  Ok(())
}

pub fn history(db: &Connection, task: TaskId) -> Result<OrganizationHistory, ServerError> {
  let mut query = db.prepare("SELECT user_id,actor,at,before,after,from_tag_names,to_tag_names FROM organization_changes WHERE task=?1 ORDER BY sequence")?;
  let mut rows = query.query([task.to_string()])?;
  let mut changes = Vec::new();
  while let Some(row) = rows.next()? {
    let user: String = row.get(0)?;
    changes.push(SharedOrganizationChange {
      by: access::user_ref(db, user.parse().map_err(|_| ServerError::StoredData)?)?,
      actor: decode(&row.get::<_, String>(1)?)?,
      at: row.get(2)?,
      from: Organization::from_core(&decode(&row.get::<_, String>(3)?)?),
      to: Organization::from_core(&decode(&row.get::<_, String>(4)?)?),
      from_tag_names: decode(&row.get::<_, String>(5)?)?,
      to_tag_names: decode(&row.get::<_, String>(6)?)?,
    });
  }
  Ok(OrganizationHistory { changes })
}
