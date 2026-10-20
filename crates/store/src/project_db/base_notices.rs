//! When the base branch moves (ORC-54, `orchester_core::base_move`): `base_heads`, where each base branch was last
//! seen; `base_notices`, the notices to runs that it moved, at most one pending per run, a newer one replacing it
//! (written by the desktop, their delivery by the runner); and `project_base_notices`, Project settings' switch for
//! them, on without a row.

use orchester_core::{
  base_move::{BaseHead, BaseNotice, LandedCommit, LandedTask, NoticeDelivery},
  id::{InvocationId, RunId, StepExecutionId, TaskId},
  revision::Revision,
  run::BranchName,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const SCHEMA: u32 = 1;
const BODY: &str = "base notice";
const COLUMNS: &str = "id, run_id, task_id, branch, revision, body, noticed_at, deliver_after, execution_id, \
                       invocation_id, delivered_at";

/// What landed, as a notice's body keeps it.
#[derive(Serialize, Deserialize)]
struct Body {
  commits: Vec<LandedCommit>,
  tasks: Vec<LandedTask>,
  overlapping: Vec<String>,
}

impl ProjectDb {
  /// Where `branch` was last seen, and when the move that put it there began; `None` before it was first seen.
  pub fn base_head(&self, branch: &BranchName) -> Result<Option<BaseHead>, StoreError> {
    let row: Option<(String, i64)> = self
      .connection
      .query_row(
        "SELECT head, moved_at FROM base_heads WHERE branch = ?1",
        [branch.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    row
      .map(|(head, moved_at)| {
        Ok(BaseHead {
          head: parse("base head", &head)?,
          moved_at: Timestamp::from_unix_millis(moved_at),
        })
      })
      .transpose()
  }

  /// Records that `branch` is at `head` now, in place of where it was seen before.
  pub fn save_base_head(&mut self, branch: &BranchName, head: &BaseHead) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO base_heads (branch, head, moved_at) VALUES (?1, ?2, ?3) \
       ON CONFLICT (branch) DO UPDATE SET head = excluded.head, moved_at = excluded.moved_at",
      params![branch.as_str(), head.head.as_str(), head.moved_at.unix_millis()],
    )?;
    Ok(())
  }

  /// Stores `notice`, a pending one, in place of its run's notice still pending, if it has one: a run has at most
  /// one pending notice, and the newer move replaces it. One the runner delivered meanwhile stays as delivered.
  pub fn queue_base_notice(&mut self, notice: &BaseNotice) -> Result<(), StoreError> {
    if !notice.is_pending() {
      return Err(corrupt("base notice", "only a pending notice is queued"));
    }
    let body = Body {
      commits: notice.commits.clone(),
      tasks: notice.tasks.clone(),
      overlapping: notice.overlapping.clone(),
    };
    self.connection.execute(
      "INSERT INTO base_notices (id, run_id, task_id, branch, revision, body, noticed_at, deliver_after) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8) \
       ON CONFLICT (run_id) WHERE delivered_at IS NULL DO UPDATE SET id = excluded.id, branch = excluded.branch, \
       revision = excluded.revision, body = excluded.body, noticed_at = excluded.noticed_at, \
       deliver_after = excluded.deliver_after",
      params![
        notice.id.to_string(),
        notice.run.to_string(),
        notice.task.to_string(),
        notice.branch.as_str(),
        notice.revision.as_str(),
        encode_json(BODY, SCHEMA, &body)?,
        notice.noticed_at.unix_millis(),
        notice.deliver_after.unix_millis(),
      ],
    )?;
    Ok(())
  }

  /// The notice `run` has pending, if it has one.
  pub fn pending_base_notice(&self, run: RunId) -> Result<Option<BaseNotice>, StoreError> {
    let sql = format!("SELECT {COLUMNS} FROM base_notices WHERE run_id = ?1 AND delivered_at IS NULL");
    let row = self
      .connection
      .query_row(&sql, [run.to_string()], read_row)
      .optional()?;
    row.map(notice_of).transpose()
  }

  /// Records that invocation `invocation` of `execution` started with `notice` at `at`, while it is still pending as
  /// it was read: `false`, with nothing stored, once a newer move replaced it or it was delivered already.
  pub fn deliver_base_notice(
    &mut self,
    notice: &BaseNotice,
    (execution, invocation): (StepExecutionId, InvocationId),
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE base_notices SET execution_id = ?1, invocation_id = ?2, delivered_at = ?3 \
       WHERE id = ?4 AND revision = ?5 AND delivered_at IS NULL",
      params![
        execution.to_string(),
        invocation.to_string(),
        at.unix_millis(),
        notice.id.to_string(),
        notice.revision.as_str(),
      ],
    )?;
    Ok(changed == 1)
  }

  /// The notices to `task`'s runs, oldest first.
  pub fn base_notices(&self, task: TaskId) -> Result<Vec<BaseNotice>, StoreError> {
    self.notices_where("task_id", &task.to_string())
  }

  /// The notices to `run`, oldest first.
  pub fn run_base_notices(&self, run: RunId) -> Result<Vec<BaseNotice>, StoreError> {
    self.notices_where("run_id", &run.to_string())
  }

  /// Whether the project tells its working agents when the base branch moves (Project settings); on until a person
  /// turned it off.
  pub fn tells_base_moves(&self) -> Result<bool, StoreError> {
    let setting: Option<String> = self
      .connection
      .query_row("SELECT setting FROM project_base_notices", [], |row| row.get(0))
      .optional()?;
    match setting.as_deref() {
      None | Some("on") => Ok(true),
      Some("off") => Ok(false),
      Some(other) => Err(corrupt("base notices setting", other)),
    }
  }

  pub fn save_tells_base_moves(&mut self, on: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO project_base_notices (id, setting) VALUES (1, ?1) \
       ON CONFLICT (id) DO UPDATE SET setting = excluded.setting",
      [if on { "on" } else { "off" }],
    )?;
    Ok(())
  }

  /// The notices whose `column` (one of this table's id columns) is `value`, oldest first.
  fn notices_where(&self, column: &str, value: &str) -> Result<Vec<BaseNotice>, StoreError> {
    let sql = format!("SELECT {COLUMNS} FROM base_notices WHERE {column} = ?1 ORDER BY noticed_at, id");
    let mut statement = self.connection.prepare(&sql)?;
    let rows = statement
      .query_map([value], read_row)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(notice_of).collect()
  }
}

/// A notice's columns, as stored.
type NoticeRow = (
  (String, String, String, String, String, String),
  (i64, i64),
  (Option<String>, Option<String>, Option<i64>),
);

fn read_row(row: &Row<'_>) -> rusqlite::Result<NoticeRow> {
  Ok((
    (
      row.get(0)?,
      row.get(1)?,
      row.get(2)?,
      row.get(3)?,
      row.get(4)?,
      row.get(5)?,
    ),
    (row.get(6)?, row.get(7)?),
    (row.get(8)?, row.get(9)?, row.get(10)?),
  ))
}

fn notice_of(
  ((id, run, task, branch, revision, body), (noticed_at, deliver_after), delivered): NoticeRow,
) -> Result<BaseNotice, StoreError> {
  let body: Body = decode_json(BODY, SCHEMA, &body)?;
  let delivery = match delivered {
    (Some(execution), Some(invocation), Some(at)) => NoticeDelivery::Delivered {
      execution: parse("step execution id", &execution)?,
      invocation: parse("invocation id", &invocation)?,
      at: Timestamp::from_unix_millis(at),
    },
    (None, None, None) => NoticeDelivery::Pending,
    _ => return Err(corrupt("base notice", "a delivery without its invocation")),
  };
  Ok(BaseNotice {
    id: parse("notice id", &id)?,
    run: parse("run id", &run)?,
    task: parse("task id", &task)?,
    branch: BranchName::new(&branch).map_err(|error| corrupt("base branch", error))?,
    revision: Revision::new(&revision).map_err(|error| corrupt("base revision", error))?,
    commits: body.commits,
    tasks: body.tasks,
    overlapping: body.overlapping,
    noticed_at: Timestamp::from_unix_millis(noticed_at),
    deliver_after: Timestamp::from_unix_millis(deliver_after),
    delivery,
  })
}
