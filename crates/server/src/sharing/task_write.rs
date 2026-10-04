use std::time::Duration;

use orchester_api::{ErrorCode, SharedTask, SharingErrorDetails, TaskChanges, TaskContent, TaskPatch};
use orchester_core::{
  id::{ProjectId, TaskId, UserId},
  name::Name,
  resource::Slug,
  task::{EditError, MissingDetail, QueueError, TaskLimits, TaskStatus, missing_details},
  tokens::TokenBudget,
};
use rusqlite::{Connection, params};

use super::{conflict, encode, events, organization, reason, sql_integer, task_rows, unsigned};
use crate::error::ServerError;

pub fn validate(content: &mut TaskContent) -> Result<(), ServerError> {
  content.title = Name::new(&content.title)
    .map_err(|_| ErrorCode::Invalid)?
    .as_str()
    .into();
  if let Some(pipeline) = &content.pipeline {
    Slug::new(pipeline).map_err(|_| ErrorCode::Invalid)?;
  }
  if let Some(limits) = &content.limits {
    TaskLimits::new(
      TokenBudget::new(limits.budget_tokens).map_err(|_| ErrorCode::Invalid)?,
      Duration::from_millis(limits.max_duration_ms),
    )
    .map_err(|_| ErrorCode::Invalid)?;
  }
  content.set_organization(
    &content
      .organization()
      .core()
      .map_err(|error| organization::failure(&error))?,
  );
  Ok(())
}

pub fn missing(content: &TaskContent) -> Vec<MissingDetail> {
  missing_details(
    &content.brief,
    &content.criteria,
    content.pipeline.is_some(),
    content.limits.is_some(),
  )
}

fn missing_names(missing: Vec<MissingDetail>) -> Vec<String> {
  missing
    .into_iter()
    .map(|detail| {
      match detail {
        MissingDetail::Brief => "brief",
        MissingDetail::AcceptanceCriteria => "criteria",
        MissingDetail::Pipeline => "pipeline",
        MissingDetail::Limits => "limits",
      }
      .into()
    })
    .collect()
}
pub fn queue_error(error: QueueError) -> ServerError {
  match error {
    QueueError::NotDraft(_) => conflict(
      ErrorCode::NotQueueable,
      SharingErrorDetails::Queue {
        missing: Vec::new(),
        reason: "not_draft".into(),
      },
    ),
    QueueError::Incomplete(missing) => conflict(
      ErrorCode::NotQueueable,
      SharingErrorDetails::Missing {
        missing: missing_names(missing),
      },
    ),
  }
}
fn edit_error(error: &EditError) -> ServerError {
  match error {
    EditError::QueuedWithoutRun => reason(ErrorCode::NotEditable, "queued_without_run"),
    EditError::Incomplete(_) => reason(ErrorCode::NotEditable, "incomplete"),
  }
}
pub const fn status_name(status: TaskStatus) -> &'static str {
  match status {
    TaskStatus::Draft => "draft",
    TaskStatus::Queued => "queued",
  }
}

pub struct NewTask {
  pub id: TaskId,
  pub number: u32,
  pub former_key: Option<String>,
  pub content: TaskContent,
  pub status: TaskStatus,
  pub created_at: i64,
  pub claim: bool,
  pub has_run: bool,
}

pub fn insert(db: &Connection, project: ProjectId, user: UserId, task: &NewTask, now: i64) -> Result<(), ServerError> {
  let limits = task.content.limits.as_ref();
  db.execute("INSERT INTO tasks(id,project,number,former_key,title,brief,criteria,pipeline,budget_tokens,max_duration_ms,status,workflow,priority,blocked_reason,revision,has_run,created_by,created_at,updated_by,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,NULL,1,?14,?15,?16,?15,?17)",
    params![task.id.to_string(),project.to_string(),task.number,task.former_key,task.content.title,task.content.brief,encode(&task.content.criteria)?,task.content.pipeline,
      limits.map(|v|v.budget_tokens.to_string()),limits.map(|v|v.max_duration_ms.to_string()),status_name(task.status),encode(&orchester_core::task::organization::WorkflowStatus::Backlog)?,encode(&orchester_core::task::organization::Priority::None)?,task.has_run || task.claim,user.to_string(),task.created_at,now])?;
  Ok(())
}

pub fn bump(db: &Connection, task: TaskId, user: UserId, now: i64) -> Result<(), ServerError> {
  let revision = unsigned(
    db.query_row("SELECT revision FROM tasks WHERE id=?1", [task.to_string()], |row| {
      row.get(0)
    })?,
  )?;
  let next = orchester_core::sharing::TaskRevision::try_from(revision)
    .map_err(|_| ServerError::StoredData)?
    .next()
    .map_err(|_| ErrorCode::Invalid)?;
  db.execute(
    "UPDATE tasks SET revision=?1,updated_by=?2,updated_at=?3 WHERE id=?4",
    params![sql_integer(next.get())?, user.to_string(), now, task.to_string()],
  )?;
  Ok(())
}

pub fn patch(
  db: &Connection,
  project: ProjectId,
  task: TaskId,
  user: UserId,
  input: TaskPatch,
  now: i64,
) -> Result<SharedTask, ServerError> {
  let old = task_rows::load(db, project, task, now)?;
  if old.revision != input.revision {
    return Err(conflict(
      ErrorCode::RevisionConflict,
      SharingErrorDetails::Revision { current: Box::new(old) },
    ));
  }
  let changes = input.changes;
  let mut content = old.content.clone();
  apply_changes(&mut content, &changes);
  validate(&mut content)?;
  let mut status = old.status;
  if let Some(requested) = changes.status {
    if requested != TaskStatus::Queued {
      return Err(ErrorCode::Invalid.into());
    }
    status.check_queue(missing(&content)).map_err(queue_error)?;
    status = requested;
  }
  // Organization is independently editable locally in ORC-39, including a waiting queued task.
  if content.title != old.content.title
    || content.brief != old.content.brief
    || content.criteria != old.content.criteria
    || content.pipeline != old.content.pipeline
    || content.limits != old.content.limits
  {
    old
      .status
      .check_edit(old.has_run, missing(&content))
      .map_err(|error| edit_error(&error))?;
  }
  let from = old.content.organization().core().map_err(|_| ServerError::StoredData)?;
  let to = content
    .organization()
    .core()
    .map_err(|error| organization::failure(&error))?;
  organization::validate(db, project, task, &from, &to)?;
  if content == old.content && status == old.status {
    return Ok(old);
  }
  if from != to {
    organization::record(
      db,
      task,
      user,
      changes
        .actor
        .unwrap_or(orchester_core::task::organization::ChangeActor::Person),
      now,
      (from, to.clone()),
    )?;
  }
  let limits = content.limits.as_ref();
  db.execute("UPDATE tasks SET title=?1,brief=?2,criteria=?3,pipeline=?4,budget_tokens=?5,max_duration_ms=?6,status=?7 WHERE id=?8",params![content.title,content.brief,encode(&content.criteria)?,content.pipeline,limits.map(|v|v.budget_tokens.to_string()),limits.map(|v|v.max_duration_ms.to_string()),status_name(status),task.to_string()])?;
  organization::save(db, task, &to)?;
  bump(db, task, user, now)?;
  events::task(db, project, task, now)?;
  task_rows::load(db, project, task, now)
}

fn apply_changes(content: &mut TaskContent, changes: &TaskChanges) {
  if let Some(title) = &changes.title {
    content.title.clone_from(title);
  }
  if let Some(brief) = &changes.brief {
    content.brief.clone_from(brief);
  }
  if let Some(criteria) = &changes.criteria {
    content.criteria.clone_from(criteria);
  }
  if let Some(pipeline) = &changes.pipeline {
    content.pipeline.clone_from(pipeline);
  }
  if let Some(limits) = &changes.limits {
    content.limits.clone_from(limits);
  }
  if let Some(workflow) = changes.workflow {
    content.workflow = workflow;
  }
  if let Some(priority) = changes.priority {
    content.priority = priority;
  }
  if let Some(tags) = &changes.tags {
    content.tags.clone_from(tags);
  }
  if let Some(blocking) = &changes.blocking {
    content.blocking.clone_from(blocking);
  }
  if let Some(blocked_reason) = &changes.blocked_reason {
    content.blocked_reason.clone_from(blocked_reason);
  }
}
