//! Plans of gated steps (`plans`), written by the runner with the run events that propose or
//! decide them, and read by the plan review.

use std::num::NonZeroU32;

use orchester_core::{
  id::RunId,
  resource::StepKey,
  run::{Plan, PlanDecision, PlanReview, PlanVersion, ReviewNote},
  time::Timestamp,
};
use rusqlite::{Connection, params};

use crate::{
  StoreError,
  rows::{corrupt, parse},
};

const APPROVED: &str = "approved";
const CHANGES_REQUESTED: &str = "changes_requested";

/// Stores a proposed plan, or the decision on a stored one. The schema keeps a plan's text and lets
/// it be decided once.
pub(super) fn save(connection: &Connection, plan: &Plan) -> Result<(), StoreError> {
  let (decision, note) = match plan.decision() {
    None => (None, None),
    Some(PlanDecision::Approved) => (Some(APPROVED), None),
    Some(PlanDecision::ChangesRequested { note }) => (Some(CHANGES_REQUESTED), Some(note.as_str())),
  };
  connection.execute(
    "INSERT INTO plans (run_id, step_key, version, step_execution_id, text, created_at, decision, note, decided_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) \
     ON CONFLICT (run_id, step_key, version) DO UPDATE SET decision = excluded.decision, note = excluded.note, \
     decided_at = excluded.decided_at",
    params![
      plan.run.to_string(),
      plan.step.as_str(),
      plan.version.get(),
      plan.execution.to_string(),
      plan.text,
      plan.created_at.unix_millis(),
      decision,
      note,
      plan.review.as_ref().map(|review| review.at.unix_millis()),
    ],
  )?;
  Ok(())
}

/// One stored row, before it is checked.
struct Row {
  step: String,
  version: i64,
  execution: String,
  text: String,
  created_at: i64,
  decision: Option<String>,
  note: Option<String>,
  decided_at: Option<i64>,
}

pub(super) fn of_run(connection: &Connection, run: RunId) -> Result<Vec<Plan>, StoreError> {
  let mut statement = connection.prepare(
    "SELECT step_key, version, step_execution_id, text, created_at, decision, note, decided_at FROM plans \
     WHERE run_id = ?1 ORDER BY step_key, version",
  )?;
  let rows = statement
    .query_map([run.to_string()], |row| {
      Ok(Row {
        step: row.get(0)?,
        version: row.get(1)?,
        execution: row.get(2)?,
        text: row.get(3)?,
        created_at: row.get(4)?,
        decision: row.get(5)?,
        note: row.get(6)?,
        decided_at: row.get(7)?,
      })
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows.into_iter().map(|row| plan(run, row)).collect()
}

fn plan(run: RunId, row: Row) -> Result<Plan, StoreError> {
  let version = u32::try_from(row.version)
    .ok()
    .and_then(NonZeroU32::new)
    .map(PlanVersion::new)
    .ok_or_else(|| corrupt("plan version", row.version))?;
  let decision = match (row.decision.as_deref(), row.note.as_deref()) {
    (None, None) => None,
    (Some(APPROVED), None) => Some(PlanDecision::Approved),
    (Some(CHANGES_REQUESTED), Some(note)) => Some(PlanDecision::ChangesRequested {
      note: ReviewNote::new(note).map_err(|error| corrupt("review note", error))?,
    }),
    (decision, note) => return Err(corrupt("plan decision", format!("{decision:?} / {note:?}"))),
  };
  let review = match (decision, row.decided_at) {
    (None, None) => None,
    (Some(decision), Some(at)) => Some(PlanReview {
      decision,
      at: Timestamp::from_unix_millis(at),
    }),
    _ => return Err(corrupt("plan decision time", format!("{:?}", row.decided_at))),
  };
  Ok(Plan {
    run,
    execution: parse("step execution id", &row.execution)?,
    step: StepKey::new(&row.step).map_err(|error| corrupt("step key", error))?,
    version,
    text: row.text,
    created_at: Timestamp::from_unix_millis(row.created_at),
    review,
  })
}
