//! `merge_queue` and `merge_queue_events`: the project's merge queue (`orchester_core::merge_queue`). Its rules are
//! `orchester-workspace`'s; this keeps the entries, their order and what happened to each run. A state, its events
//! and the checks a run passed are JSON documents ([`encode_json`]).

use orchester_core::{
  id::{RunId, TaskId},
  merge_queue::{CheckedOn, QueueEntry, QueueEvent, QueueState, RecordedQueueEvent},
  run::BranchName,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{decode_json, encode_json, parse},
};

const STATE: (&str, u32) = ("merge queue state", 1);
const CHECKED: (&str, u32) = ("merge queue check", 1);
const EVENT: (&str, u32) = ("merge queue event", 1);

const COLUMNS: &str = "run_id, task_id, finished_at, position, state, since, resyncs, checked, approved_at";

/// [`COLUMNS`] as read, with the base branch the entry's run recorded: the entry's lane.
const READ_COLUMNS: &str = "run_id, task_id, finished_at, position, state, since, resyncs, checked, approved_at, \
                            (SELECT r.base_branch FROM runs r WHERE r.id = merge_queue.run_id)";

impl ProjectDb {
  /// Every entry of the merge queue, the runs that left it or were merged included, in the queue's order.
  pub fn merge_queue(&self) -> Result<Vec<QueueEntry>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT {READ_COLUMNS} FROM merge_queue ORDER BY position, finished_at"
    ))?;
    let rows = statement.query_map([], read_row)?;
    rows.map(|row| entry(row?)).collect()
  }

  /// The entry of `run`, if it ever joined the queue.
  pub fn merge_queue_entry(&self, run: RunId) -> Result<Option<QueueEntry>, StoreError> {
    stored_entry(&self.connection, run)
  }

  /// Puts `run`, of `task`, which passed every step at `finished_at`, at the end of the queue as Queued at `now`,
  /// unless it has an entry already; whether it joined.
  pub fn join_merge_queue(
    &mut self,
    (run, task): (RunId, TaskId),
    finished_at: Timestamp,
    now: Timestamp,
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    if stored_entry(&transaction, run)?.is_some() {
      return Ok(false);
    }
    let last: Option<i64> = transaction.query_row("SELECT MAX(position) FROM merge_queue", [], |row| row.get(0))?;
    transaction.execute(
      &format!("INSERT INTO merge_queue ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, NULL)"),
      params![
        run.to_string(),
        task.to_string(),
        finished_at.unix_millis(),
        last.map_or(1, |last| last + 1),
        encode_json(STATE.0, STATE.1, &QueueState::Queued)?,
        now.unix_millis(),
      ],
    )?;
    append(&transaction, run, now, &[QueueEvent::Joined])?;
    transaction.commit()?;
    Ok(true)
  }

  /// Saves where `entry` stands now (its state, since when, its re-syncs, its last passed check and its approval;
  /// not its place in the order) with `events`, which happened at `now`; `false`, with nothing saved, once its run
  /// is no longer in the queue's table (a person removed it meanwhile) or its state is no longer `expected`.
  pub fn save_merge_queue_entry(
    &mut self,
    entry: &QueueEntry,
    expected: &QueueState,
    (events, now): (&[QueueEvent], Timestamp),
  ) -> Result<bool, StoreError> {
    let transaction = self.connection.transaction()?;
    let stored = stored_entry(&transaction, entry.run)?;
    if stored.as_ref().map(|stored| &stored.state) != Some(expected) {
      return Ok(false);
    }
    let checked = entry
      .checked
      .as_ref()
      .map(|checked| encode_json(CHECKED.0, CHECKED.1, checked))
      .transpose()?;
    transaction.execute(
      "UPDATE merge_queue SET state = ?2, since = ?3, resyncs = ?4, checked = ?5, approved_at = ?6 WHERE run_id = ?1",
      params![
        entry.run.to_string(),
        encode_json(STATE.0, STATE.1, &entry.state)?,
        entry.since.unix_millis(),
        entry.resyncs,
        checked,
        entry.approved_at.map(Timestamp::unix_millis),
      ],
    )?;
    append(&transaction, entry.run, now, events)?;
    transaction.commit()?;
    Ok(true)
  }

  /// Gives the runs of `order` the places they are listed in, after every place taken now; the others keep theirs.
  pub fn order_merge_queue(&mut self, order: &[RunId]) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let last: Option<i64> = transaction.query_row("SELECT MAX(position) FROM merge_queue", [], |row| row.get(0))?;
    let mut position = last.unwrap_or(0);
    for run in order {
      position += 1;
      transaction.execute(
        "UPDATE merge_queue SET position = ?2 WHERE run_id = ?1",
        params![run.to_string(), position],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }

  /// The tasks' latest runs that succeeded and never joined the queue, whose pipeline (as the run pinned it) ends in a
  /// Deliver step, whose work was not merged and whose worktree is there, with their tasks and when they last
  /// succeeded, first finished first: the runs that join it.
  pub fn merge_queue_candidates(&self) -> Result<Vec<(RunId, TaskId, Timestamp)>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT r.id, r.task_id,
         (SELECT MAX(e.at) FROM run_events e WHERE e.run_id = r.id AND e.kind = 'succeeded') AS finished
       FROM runs r
       WHERE r.status = 'succeeded'
         AND json_extract(r.snapshot, '$.body.body.deliver') IS NOT NULL
         AND NOT EXISTS (SELECT 1 FROM runs n WHERE n.task_id = r.task_id AND n.number > r.number)
         AND NOT EXISTS (SELECT 1 FROM run_merges m WHERE m.run_id = r.id)
         AND NOT EXISTS (SELECT 1 FROM run_worktree_removals w WHERE w.run_id = r.id)
         AND NOT EXISTS (SELECT 1 FROM merge_queue q WHERE q.run_id = r.id)
       ORDER BY finished, r.number",
    )?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, Option<i64>>(2)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(run, task, finished)| {
        Ok((
          parse("run id", &run)?,
          parse("task id", &task)?,
          Timestamp::from_unix_millis(finished.unwrap_or_default()),
        ))
      })
      .collect()
  }

  /// What happened to `run` in the merge queue, oldest first.
  pub fn merge_queue_events(&self, run: RunId) -> Result<Vec<RecordedQueueEvent>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT at, event FROM merge_queue_events WHERE run_id = ?1 ORDER BY sequence")?;
    let rows = statement.query_map([run.to_string()], |row| {
      Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    rows
      .map(|row| {
        let (at, event) = row?;
        Ok(RecordedQueueEvent {
          run,
          at: Timestamp::from_unix_millis(at),
          event: decode_json(EVENT.0, EVENT.1, &event)?,
        })
      })
      .collect()
  }
}

/// The columns of a row, as read ([`READ_COLUMNS`]).
type Stored = (
  String,
  String,
  i64,
  i64,
  String,
  i64,
  i64,
  Option<String>,
  Option<i64>,
  Option<String>,
);

fn read_row(row: &Row<'_>) -> rusqlite::Result<Stored> {
  Ok((
    row.get(0)?,
    row.get(1)?,
    row.get(2)?,
    row.get(3)?,
    row.get(4)?,
    row.get(5)?,
    row.get(6)?,
    row.get(7)?,
    row.get(8)?,
    row.get(9)?,
  ))
}

fn entry(
  (run, task, finished_at, position, state, since, resyncs, checked, approved_at, base): Stored,
) -> Result<QueueEntry, StoreError> {
  let checked: Option<CheckedOn> = checked
    .map(|checked| decode_json(CHECKED.0, CHECKED.1, &checked))
    .transpose()?;
  Ok(QueueEntry {
    run: parse("run id", &run)?,
    task: parse("task id", &task)?,
    base: base
      .as_deref()
      .map(BranchName::new)
      .transpose()
      .map_err(|error| crate::rows::corrupt("merge queue base branch", error))?,
    finished_at: Timestamp::from_unix_millis(finished_at),
    position,
    state: decode_json(STATE.0, STATE.1, &state)?,
    since: Timestamp::from_unix_millis(since),
    resyncs: u32::try_from(resyncs).map_err(|_| crate::rows::corrupt("merge queue re-syncs", resyncs))?,
    checked,
    approved_at: approved_at.map(Timestamp::from_unix_millis),
  })
}

fn stored_entry(connection: &Connection, run: RunId) -> Result<Option<QueueEntry>, StoreError> {
  connection
    .query_row(
      &format!("SELECT {READ_COLUMNS} FROM merge_queue WHERE run_id = ?1"),
      [run.to_string()],
      read_row,
    )
    .optional()?
    .map(entry)
    .transpose()
}

/// Appends `events` of `run`, at `at`, after its last one.
fn append(connection: &Connection, run: RunId, at: Timestamp, events: &[QueueEvent]) -> Result<(), StoreError> {
  let last: Option<i64> = connection.query_row(
    "SELECT MAX(sequence) FROM merge_queue_events WHERE run_id = ?1",
    [run.to_string()],
    |row| row.get(0),
  )?;
  let mut sequence = last.unwrap_or(0);
  for event in events {
    sequence += 1;
    connection.execute(
      "INSERT INTO merge_queue_events (run_id, sequence, at, event) VALUES (?1, ?2, ?3, ?4)",
      params![
        run.to_string(),
        sequence,
        at.unix_millis(),
        encode_json(EVENT.0, EVENT.1, event)?
      ],
    )?;
  }
  Ok(())
}
