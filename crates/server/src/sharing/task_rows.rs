use orchester_api::{Attribution, ErrorCode, RunStatus, SharedLimits, SharedTask, TaskContent};
use orchester_core::{
  id::{ProjectId, TaskId, UserId},
  project::{ProjectKey, TaskKey},
  sharing::{Claim, TaskRevision},
  task::TaskStatus,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{access, claims, decode, organization, unsigned};
use crate::error::ServerError;

pub fn ids(db: &Connection, project: ProjectId) -> Result<Vec<TaskId>, ServerError> {
  let mut query = db.prepare("SELECT id FROM tasks WHERE project=?1 ORDER BY number")?;
  let ids = query
    .query_map([project.to_string()], |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  ids
    .into_iter()
    .map(|id| id.parse().map_err(|_| ServerError::StoredData))
    .collect()
}

pub fn project_of(db: &Connection, task: TaskId) -> Result<Option<ProjectId>, ServerError> {
  let project: Option<String> = db
    .query_row("SELECT project FROM tasks WHERE id=?1", [task.to_string()], |row| {
      row.get(0)
    })
    .optional()?;
  project
    .map(|id| id.parse().map_err(|_| ServerError::StoredData))
    .transpose()
}

pub fn load(db: &Connection, project: ProjectId, task: TaskId, now: i64) -> Result<SharedTask, ServerError> {
  let mut query = db.prepare("SELECT t.number,p.key,t.former_key,t.revision,t.title,t.brief,t.criteria,t.pipeline,t.budget_tokens,t.max_duration_ms,t.status,t.has_run,t.created_by,t.created_at,t.updated_by,t.updated_at FROM tasks t JOIN projects p ON p.id=t.project WHERE t.id=?1 AND t.project=?2")?;
  let mut rows = query.query(params![task.to_string(), project.to_string()])?;
  let row = rows.next()?.ok_or(ErrorCode::NotFound)?;
  let status: String = row.get(10)?;
  let number: u32 = row.get(0)?;
  let key: String = row.get(1)?;
  let created: String = row.get(12)?;
  let updated: String = row.get(14)?;
  let mut content = TaskContent {
    title: row.get(4)?,
    brief: row.get(5)?,
    criteria: decode(&row.get::<_, String>(6)?)?,
    pipeline: row.get(7)?,
    limits: limits(row)?,
    workflow: Default::default(),
    priority: Default::default(),
    tags: Vec::new(),
    blocking: Vec::new(),
    blocked_reason: None,
  };
  content.set_organization(&organization::load(db, task)?);
  Ok(SharedTask {
    id: task,
    number,
    key: TaskKey::new(
      ProjectKey::new(&key).map_err(|_| ServerError::StoredData)?,
      std::num::NonZeroU32::new(number).ok_or(ServerError::StoredData)?,
    )
    .to_string(),
    former_key: row.get(2)?,
    revision: TaskRevision::try_from(unsigned(row.get(3)?)?).map_err(|_| ServerError::StoredData)?,
    content,
    status: match status.as_str() {
      "draft" => TaskStatus::Draft,
      "queued" => TaskStatus::Queued,
      _ => return Err(ServerError::StoredData),
    },
    has_run: row.get(11)?,
    created: Attribution {
      by: access::user_ref(db, created.parse().map_err(|_| ServerError::StoredData)?)?,
      at: row.get(13)?,
    },
    updated: Attribution {
      by: access::user_ref(db, updated.parse().map_err(|_| ServerError::StoredData)?)?,
      at: row.get(15)?,
    },
    claim: claim(db, task)?
      .as_ref()
      .map(|claim| claims::snapshot(db, claim, now))
      .transpose()?,
    run: run(db, task)?,
  })
}

fn limits(row: &rusqlite::Row<'_>) -> Result<Option<SharedLimits>, ServerError> {
  // Decimal text preserves core's full u64 range without SQLite's REAL coercion.
  let budget = row
    .get::<_, Option<String>>(8)?
    .map(|value| value.parse::<u64>().map_err(|_| ServerError::StoredData))
    .transpose()?;
  let duration = row
    .get::<_, Option<String>>(9)?
    .map(|value| value.parse::<u64>().map_err(|_| ServerError::StoredData))
    .transpose()?;
  match (budget, duration) {
    (Some(budget_tokens), Some(max_duration_ms)) => Ok(Some(SharedLimits {
      budget_tokens,
      max_duration_ms,
    })),
    (None, None) => Ok(None),
    _ => Err(ServerError::StoredData),
  }
}

pub fn claim(db: &Connection, task: TaskId) -> Result<Option<Claim>, ServerError> {
  let value: Option<String> = db
    .query_row("SELECT json_object('id',id,'holder',holder,'kind',kind,'since',since,'heartbeat_at',heartbeat_at,'taken_over_from',taken_over_from) FROM claims WHERE task=?1", [task.to_string()], |row| {
      row.get(0)
    })
    .optional()?;
  value.map(|value| decode(&value)).transpose()
}

pub fn run(db: &Connection, task: TaskId) -> Result<Option<RunStatus>, ServerError> {
  let mut query =
    db.prepare("SELECT holder,number,phase,step,started_at,phase_since,updated_at FROM run_status WHERE task=?1")?;
  let mut rows = query.query([task.to_string()])?;
  let Some(row) = rows.next()? else {
    return Ok(None);
  };
  let holder: String = row.get(0)?;
  let phase: String = row.get(2)?;
  // Use the wire adapter even for stored reports; no independent phase vocabulary.
  #[derive(serde::Deserialize)]
  struct Phase(#[serde(with = "orchester_api::wire::Phase")] orchester_core::run::TaskPhase);
  Ok(Some(RunStatus {
    holder: access::user_ref(db, holder.parse::<UserId>().map_err(|_| ServerError::StoredData)?)?,
    number: row.get(1)?,
    phase: decode::<Phase>(&serde_json::to_string(&phase).map_err(|_| ServerError::StoredData)?)?.0,
    step: row.get(3)?,
    started_at: row.get(4)?,
    phase_since: row.get(5)?,
    updated_at: row.get(6)?,
  }))
}
