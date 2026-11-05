//! `merge_attempts` and `merge_attempts_open`: every merge attempt of the project's runs, and the attempts that passed
//! their preflight and have not ended (`orchester_core::integration::attempt`). An attempt's outcome and stages are
//! JSON documents ([`encode_json`]).

use std::num::NonZeroU32;

use orchester_core::{
  id::RunId,
  integration::attempt::{
    AttemptOutcome, AttemptStage, CauseKind, MergeAsker, MergeAttempt, MergeVia, OpenAttempt, RecordedAttempt,
    StageResult,
  },
  time::Timestamp,
};
use rusqlite::{Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

const OUTCOME: (&str, u32) = ("merge attempt outcome", 1);
const STAGES: (&str, u32) = ("merge attempt stages", 1);
const CAUSE_KIND: (&str, u32) = ("merge attempt cause kind", 1);

const COLUMNS: &str = "task_id, sequence, run_id, at, asked_by, retry, via, detail, cause, stages, cause_kind";

impl ProjectDb {
  /// Records `attempt` after its run's others and ends its open attempt, if it has one; its place among them.
  pub fn record_merge_attempt(&mut self, attempt: &MergeAttempt) -> Result<u32, StoreError> {
    let transaction = self.connection.transaction()?;
    let run = attempt.run().to_string();
    let last: Option<u32> = transaction.query_row(
      "SELECT MAX(sequence) FROM merge_attempts WHERE run_id = ?1",
      [&run],
      |row| row.get(0),
    )?;
    let sequence = last.unwrap_or(0) + 1;
    let (asked_by, retry) = asker_text(attempt.asked_by());
    let kind = attempt
      .kind()
      .map(|kind| encode_json(CAUSE_KIND.0, CAUSE_KIND.1, &kind))
      .transpose()?;
    let inserted = transaction.execute(
      "INSERT INTO merge_attempts
         (run_id, sequence, task_id, at, asked_by, retry, via, outcome, detail, cause, stages, cause_kind)
       SELECT id, ?2, task_id, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11 FROM runs WHERE id = ?1",
      params![
        run,
        sequence,
        attempt.at().unix_millis(),
        asked_by,
        retry,
        via_text(attempt.via()),
        attempt.outcome().name(),
        encode_json(OUTCOME.0, OUTCOME.1, attempt.outcome())?,
        attempt.cause(),
        encode_json(STAGES.0, STAGES.1, &attempt.stages())?,
        kind,
      ],
    )?;
    if inserted == 0 {
      return Err(StoreError::RunNotFound(attempt.run()));
    }
    transaction.execute("DELETE FROM merge_attempts_open WHERE run_id = ?1", [&run])?;
    transaction.commit()?;
    Ok(sequence)
  }

  /// Keeps `open` as its run's attempt that has not ended, at the stage it names, in place of what was kept.
  pub fn open_merge_attempt(&mut self, open: &OpenAttempt) -> Result<(), StoreError> {
    let (asked_by, retry) = asker_text(open.asked_by);
    self.connection.execute(
      "INSERT OR REPLACE INTO merge_attempts_open (run_id, at, asked_by, retry, via, stage)
       VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
      params![
        open.run.to_string(),
        open.at.unix_millis(),
        asked_by,
        retry,
        via_text(open.via),
        open.stage.name(),
      ],
    )?;
    Ok(())
  }

  /// Ends `run`'s open attempt without a record of its own: it repeats the run's last recorded attempt.
  pub fn close_merge_attempt(&mut self, run: RunId) -> Result<(), StoreError> {
    self
      .connection
      .execute("DELETE FROM merge_attempts_open WHERE run_id = ?1", [run.to_string()])?;
    Ok(())
  }

  /// The attempts that passed their preflight and were not recorded, oldest first.
  pub fn open_merge_attempts(&self) -> Result<Vec<OpenAttempt>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT run_id, at, asked_by, retry, via, stage FROM merge_attempts_open ORDER BY at")?;
    let rows = statement.query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, i64>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, Option<u32>>(3)?,
        row.get::<_, String>(4)?,
        row.get::<_, String>(5)?,
      ))
    })?;
    rows
      .map(|row| {
        let (run, at, asked_by, retry, via_name, stage) = row?;
        Ok(OpenAttempt {
          run: parse("run id", &run)?,
          at: Timestamp::from_unix_millis(at),
          asked_by: asker(&asked_by, retry)?,
          via: via(&via_name)?,
          stage: stage_named(&stage)?,
        })
      })
      .collect()
  }

  /// `run`'s attempts, newest first.
  pub fn merge_attempts(&self, run: RunId) -> Result<Vec<RecordedAttempt>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM merge_attempts WHERE run_id = ?1 ORDER BY sequence DESC"
    ))?;
    let rows = statement.query_map([run.to_string()], read_row)?;
    rows.map(|row| recorded(row?)).collect()
  }

  /// The last attempt of each run whose last attempt did not land and that was not merged since, newest first.
  pub fn unlanded_merge_attempts(&self) -> Result<Vec<RecordedAttempt>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM merge_attempts a
       WHERE a.sequence = (SELECT MAX(l.sequence) FROM merge_attempts l WHERE l.run_id = a.run_id)
         AND a.outcome <> 'merged'
         AND NOT EXISTS (SELECT 1 FROM run_merges m WHERE m.run_id = a.run_id)
       ORDER BY a.at DESC, a.run_id"
    ))?;
    let rows = statement.query_map([], read_row)?;
    rows.map(|row| recorded(row?)).collect()
  }
}

/// The columns of a row, as read ([`COLUMNS`]).
type Stored = (
  String,
  u32,
  String,
  i64,
  String,
  Option<u32>,
  String,
  String,
  Option<String>,
  String,
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
    row.get(10)?,
  ))
}

fn recorded(
  (task, sequence, run, at, asked_by, retry, via_name, detail, cause, stages, kind): Stored,
) -> Result<RecordedAttempt, StoreError> {
  let outcome: AttemptOutcome = decode_json(OUTCOME.0, OUTCOME.1, &detail)?;
  let stages: Vec<(AttemptStage, StageResult)> = decode_json(STAGES.0, STAGES.1, &stages)?;
  let kind: Option<CauseKind> = kind
    .map(|kind| decode_json(CAUSE_KIND.0, CAUSE_KIND.1, &kind))
    .transpose()?;
  Ok(RecordedAttempt {
    task: parse("task id", &task)?,
    sequence,
    attempt: MergeAttempt::recorded(
      (parse("run id", &run)?, Timestamp::from_unix_millis(at)),
      (asker(&asked_by, retry)?, via(&via_name)?),
      (outcome, cause, kind),
      stages,
    ),
  })
}

const fn asker_text(asker: MergeAsker) -> (&'static str, Option<u32>) {
  match asker {
    MergeAsker::Person => ("person", None),
    MergeAsker::Automation => ("automation", None),
    MergeAsker::MergeQueue => ("merge_queue", None),
    MergeAsker::Retry { number } => ("retry", Some(number.get())),
  }
}

fn asker(text: &str, retry: Option<u32>) -> Result<MergeAsker, StoreError> {
  Ok(match (text, retry) {
    ("person", None) => MergeAsker::Person,
    ("automation", None) => MergeAsker::Automation,
    ("merge_queue", None) => MergeAsker::MergeQueue,
    ("retry", Some(number)) => MergeAsker::Retry {
      number: NonZeroU32::new(number).ok_or_else(|| corrupt("merge attempt retry", number))?,
    },
    (other, _) => return Err(corrupt("merge attempt asker", other)),
  })
}

const fn via_text(via: MergeVia) -> &'static str {
  match via {
    MergeVia::TaskPage => "task_page",
    MergeVia::MergeRun => "merge_run",
    MergeVia::MergeQueue => "merge_queue",
  }
}

fn via(text: &str) -> Result<MergeVia, StoreError> {
  Ok(match text {
    "task_page" => MergeVia::TaskPage,
    "merge_run" => MergeVia::MergeRun,
    "merge_queue" => MergeVia::MergeQueue,
    other => return Err(corrupt("merge attempt via", other)),
  })
}

fn stage_named(text: &str) -> Result<AttemptStage, StoreError> {
  serde_json::from_value(serde_json::Value::String(text.to_owned())).map_err(|_| corrupt("merge attempt stage", text))
}
