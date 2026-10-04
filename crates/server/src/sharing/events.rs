use orchester_api::EventKind;
use orchester_core::id::ProjectId;
use rusqlite::{Connection, params};
use serde::Serialize;

use super::{encode, sql_integer, task_rows, unsigned};
use crate::error::ServerError;

pub const RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;

pub fn append(
  db: &Connection,
  project: ProjectId,
  kind: EventKind,
  payload: &impl Serialize,
  now: i64,
) -> Result<(), ServerError> {
  db.execute(
    "INSERT INTO events(project,kind,payload,at) VALUES (?1,?2,?3,?4)",
    params![project.to_string(), kind.as_str(), encode(payload)?, now],
  )?;
  Ok(())
}

pub fn task(
  db: &Connection,
  project: ProjectId,
  task: orchester_core::id::TaskId,
  now: i64,
) -> Result<(), ServerError> {
  append(
    db,
    project,
    EventKind::Task,
    &task_rows::load(db, project, task, now)?,
    now,
  )
}

pub fn prune(db: &Connection, now: i64) -> Result<(), ServerError> {
  let cutoff = now.saturating_sub(RETENTION_MS);
  db.execute("UPDATE projects SET event_floor=MAX(event_floor,COALESCE((SELECT MAX(sequence) FROM events INDEXED BY events_project_retention WHERE project=projects.id AND at<?1),0))",[cutoff])?;
  db.execute("DELETE FROM events WHERE at<?1", [cutoff])?;
  Ok(())
}

pub fn cursor(db: &Connection, project: ProjectId) -> Result<u64, ServerError> {
  unsigned(db.query_row(
    "SELECT MAX(event_floor,COALESCE((SELECT MAX(sequence) FROM events WHERE project=?1),0)) FROM projects WHERE id=?1",
    [project.to_string()],
    |row| row.get(0),
  )?)
}

pub struct Event {
  pub id: u64,
  pub kind: EventKind,
  pub payload: String,
}

pub struct Replay {
  pub reset: bool,
  pub events: Vec<Event>,
}

pub fn replay(db: &Connection, project: ProjectId, after: u64, now: i64) -> Result<Replay, ServerError> {
  let cutoff = now.saturating_sub(RETENTION_MS);
  let floor = unsigned(db.query_row("SELECT MAX(event_floor,COALESCE((SELECT MAX(sequence) FROM events INDEXED BY events_project_retention WHERE project=?1 AND at<?2),0)) FROM projects WHERE id=?1",params![project.to_string(),cutoff],|row|row.get(0))?)?;
  if after < floor {
    return Ok(Replay {
      reset: true,
      events: Vec::new(),
    });
  }
  // Batch small snapshots while bounding both event count and payload memory for slow clients.
  let mut query = db.prepare(
    "SELECT sequence,kind,payload FROM events WHERE project=?1 AND sequence>?2 AND at>=?3 ORDER BY sequence LIMIT 64",
  )?;
  let mut rows = query.query(params![project.to_string(), sql_integer(after)?, cutoff])?;
  let mut events = Vec::new();
  let mut bytes = 0;
  while let Some(row) = rows.next()? {
    let payload: String = row.get(2)?;
    if !events.is_empty() && bytes + payload.len() > 1024 * 1024 {
      break;
    }
    bytes += payload.len();
    let kind: String = row.get(1)?;
    events.push(Event {
      id: unsigned(row.get(0)?)?,
      kind: EventKind::try_from(kind.as_str()).map_err(|_| ServerError::StoredData)?,
      payload,
    });
  }
  Ok(Replay { reset: false, events })
}
