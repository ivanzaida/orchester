use std::collections::HashSet;

use orchester_api::{DroppedLink, ErrorCode, TaskContent, TasksCreated, UploadTask};
use orchester_core::{
  id::{IdempotencyKey, ProjectId, TaskId, UserId},
  task::{
    TaskStatus,
    organization::{ChangeActor, TaskOrganization},
  },
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{
  claims, decode, encode, events, organization, tags, task_rows,
  task_write::{self, NewTask},
};
use crate::error::ServerError;

pub fn repeated(db: &Connection, project: ProjectId, key: IdempotencyKey) -> Result<Option<TasksCreated>, ServerError> {
  let value: Option<String> = db
    .query_row(
      "SELECT response FROM task_batches WHERE project=?1 AND key=?2",
      params![project.to_string(), key.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  value.map(|value| decode(&value)).transpose()
}

pub fn remember(
  db: &Connection,
  project: ProjectId,
  key: IdempotencyKey,
  user: UserId,
  response: &TasksCreated,
) -> Result<(), ServerError> {
  db.execute(
    "INSERT INTO task_batches VALUES (?1,?2,?3,?4)",
    params![
      project.to_string(),
      key.to_string(),
      user.to_string(),
      encode(response)?
    ],
  )?;
  Ok(())
}

fn counter(db: &Connection, project: ProjectId) -> Result<u64, ServerError> {
  super::unsigned(db.query_row(
    "SELECT next_task_number FROM projects WHERE id=?1",
    [project.to_string()],
    |row| row.get(0),
  )?)
}

pub fn create(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  key: IdempotencyKey,
  contents: Vec<TaskContent>,
  now: i64,
) -> Result<TasksCreated, ServerError> {
  if let Some(response) = repeated(db, project, key)? {
    return Ok(response);
  }
  let mut next = counter(db, project)?;
  let mut tasks = Vec::new();
  for mut content in contents {
    task_write::validate(&mut content)?;
    tasks.push(NewTask {
      id: TaskId::generate(),
      number: u32::try_from(next).map_err(|_| ErrorCode::Invalid)?,
      former_key: None,
      content,
      status: TaskStatus::Draft,
      created_at: now,
      claim: false,
    });
    next = next.checked_add(1).ok_or(ErrorCode::Invalid)?;
  }
  let response = insert(
    db,
    project,
    user,
    PreparedBatch {
      tasks,
      next,
      dropped_links: Vec::new(),
      uploaded: false,
    },
    now,
  )?;
  remember(db, project, key, user, &response)?;
  Ok(response)
}

pub fn import(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  key: IdempotencyKey,
  input: Vec<UploadTask>,
  now: i64,
) -> Result<TasksCreated, ServerError> {
  if let Some(response) = repeated(db, project, key)? {
    return Ok(response);
  }
  let response = upload(db, project, user, input, false, now)?;
  remember(db, project, key, user, &response)?;
  Ok(response)
}

pub fn upload(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  input: Vec<UploadTask>,
  keep_numbers: bool,
  now: i64,
) -> Result<TasksCreated, ServerError> {
  let mut known: HashSet<_> = task_rows::ids(db, project)?.into_iter().collect();
  let mut incoming = HashSet::new();
  for task in &input {
    let exists: bool = db.query_row(
      "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1)",
      [task.id.to_string()],
      |row| row.get(0),
    )?;
    if exists || !incoming.insert(task.id) {
      return Err(ErrorCode::IdTaken.into());
    }
  }
  known.extend(incoming);
  let mut next = counter(db, project)?;
  if keep_numbers {
    for task in &input {
      if let Some(number) = task.number {
        next = next.max(u64::from(number) + 1);
      }
    }
  }
  let mut used = HashSet::new();
  let mut tasks = Vec::new();
  let mut dropped = Vec::new();
  for task in input {
    let number = if keep_numbers {
      task.number.map(u64::from).unwrap_or(next)
    } else {
      next
    };
    let number = u32::try_from(number).map_err(|_| ErrorCode::Invalid)?;
    if number == 0 || !used.insert(number) {
      return Err(ErrorCode::Invalid.into());
    }
    if u64::from(number) >= next {
      next = u64::from(number) + 1;
    }
    let content = upload_content(db, project, task, &known, &mut dropped)?;
    tasks.push(NewTask { number, ..content });
  }
  insert(
    db,
    project,
    user,
    PreparedBatch {
      tasks,
      next,
      dropped_links: dropped,
      uploaded: true,
    },
    now,
  )
}

fn upload_content(
  db: &Connection,
  project: ProjectId,
  task: UploadTask,
  known: &HashSet<TaskId>,
  dropped: &mut Vec<DroppedLink>,
) -> Result<NewTask, ServerError> {
  let tags = tags::upload_names(db, project, task.tag_names)?;
  let blocking = task
    .blocking
    .into_iter()
    .filter(|link| {
      if known.contains(&link.task) {
        true
      } else {
        dropped.push(DroppedLink {
          task: task.id,
          blocking: link.task,
        });
        false
      }
    })
    .collect();
  let mut content = TaskContent {
    title: task.title,
    brief: task.brief,
    criteria: task.criteria,
    pipeline: task.pipeline,
    limits: task.limits,
    workflow: task.workflow,
    priority: task.priority,
    tags,
    blocking,
    blocked_reason: task.blocked_reason,
  };
  task_write::validate(&mut content)?;
  if task.status == TaskStatus::Queued || task.claim {
    TaskStatus::Draft
      .check_queue(task_write::missing(&content))
      .map_err(task_write::queue_error)?;
  }
  if let Some(key) = &task.former_key {
    let (project, number) = key.rsplit_once('-').ok_or(ErrorCode::Invalid)?;
    orchester_core::project::ProjectKey::new(project).map_err(|_| ErrorCode::Invalid)?;
    number.parse::<std::num::NonZeroU32>().map_err(|_| ErrorCode::Invalid)?;
  }
  Ok(NewTask {
    id: task.id,
    number: 0,
    former_key: task.former_key,
    content,
    status: task.status,
    created_at: task.created_at,
    claim: task.claim,
  })
}

struct PreparedBatch {
  tasks: Vec<NewTask>,
  next: u64,
  dropped_links: Vec<DroppedLink>,
  uploaded: bool,
}

fn insert(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  batch: PreparedBatch,
  now: i64,
) -> Result<TasksCreated, ServerError> {
  let PreparedBatch {
    tasks,
    next,
    dropped_links,
    uploaded,
  } = batch;
  for task in &tasks {
    task_write::insert(db, project, user, task, now)?;
  }
  for task in &tasks {
    let value = task
      .content
      .organization()
      .core()
      .map_err(|error| organization::failure(&error))?;
    organization::save(db, task.id, &value)?;
  }
  let mut snapshots = Vec::new();
  for task in tasks {
    let value = task
      .content
      .organization()
      .core()
      .map_err(|error| organization::failure(&error))?;
    organization::validate(db, project, task.id, &TaskOrganization::default(), &value)?;
    if uploaded || value != TaskOrganization::default() {
      organization::record(
        db,
        task.id,
        user,
        ChangeActor::Person,
        now,
        (TaskOrganization::default(), value),
      )?;
    }
    if task.claim {
      claims::upload(db, task.id, user, now)?;
    }
    events::task(db, project, task.id, now)?;
    snapshots.push(task_rows::load(db, project, task.id, now)?);
  }
  db.execute(
    "UPDATE projects SET next_task_number=?1 WHERE id=?2",
    params![super::sql_integer(next)?, project.to_string()],
  )?;
  if uploaded {
    tags::event(db, project, now)?;
  }
  Ok(TasksCreated {
    tasks: snapshots,
    dropped_links,
  })
}
