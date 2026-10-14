//! Outputs of step executions (`step_outputs`), written by the runner when an execution ends and
//! read into the prompts of the steps that run after it.

use orchester_core::{
  id::RunId,
  run::{StepOutcome, StepOutput},
  time::Timestamp,
};
use rusqlite::{Connection, params};

use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

pub(super) fn save(connection: &Connection, output: &StepOutput) -> Result<(), StoreError> {
  connection.execute(
    "INSERT INTO step_outputs (step_execution_id, run_id, step_key, outcome, text, created_at, unverified) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) ON CONFLICT (step_execution_id) DO NOTHING",
    params![
      output.execution.to_string(),
      output.run.to_string(),
      output.step.as_str(),
      output.outcome.map(outcome_text).transpose()?,
      output.text,
      output.created_at.unix_millis(),
      unverified_column(output)?,
    ],
  )?;
  Ok(())
}

pub(super) fn of_run(connection: &Connection, run: RunId) -> Result<Vec<StepOutput>, StoreError> {
  let mut statement = connection.prepare(
    "SELECT step_execution_id, step_key, outcome, text, created_at, unverified FROM step_outputs WHERE run_id = ?1 \
     ORDER BY created_at, rowid",
  )?;
  let rows = statement
    .query_map([run.to_string()], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, Option<String>>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, i64>(4)?,
        row.get::<_, Option<String>>(5)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(execution, step, outcome, text, created_at, unverified)| {
      Ok(StepOutput {
        run,
        execution: parse("step execution id", &execution)?,
        step: orchester_core::resource::StepKey::new(&step).map_err(|error| corrupt("step key", error))?,
        outcome: outcome.as_deref().map(parse_outcome).transpose()?,
        text,
        unverified: unverified
          .map(|list| decode_json(UNVERIFIED, UNVERIFIED_SCHEMA, &list))
          .transpose()?
          .unwrap_or_default(),
        created_at: Timestamp::from_unix_millis(created_at),
      })
    })
    .collect()
}

/// What the `unverified` column holds: the checks an `unverified` output could not do.
const UNVERIFIED: &str = "unverified checks";
const UNVERIFIED_SCHEMA: u32 = 1;

/// The `unverified` column of `output`: its list for an `unverified` outcome, none for any other.
fn unverified_column(output: &StepOutput) -> Result<Option<String>, StoreError> {
  if output.outcome != Some(StepOutcome::Unverified) {
    return Ok(None);
  }
  encode_json(UNVERIFIED, UNVERIFIED_SCHEMA, &output.unverified).map(Some)
}

/// A question is no output: its step waits for the answer and goes on.
fn outcome_text(outcome: StepOutcome) -> Result<&'static str, StoreError> {
  match outcome {
    StepOutcome::Done => Ok("done"),
    StepOutcome::Failed => Ok("failed"),
    StepOutcome::Unverified => Ok("unverified"),
    StepOutcome::Question => Err(StoreError::Unrepresentable("a question as a step's output")),
  }
}

fn parse_outcome(value: &str) -> Result<StepOutcome, StoreError> {
  match value {
    "done" => Ok(StepOutcome::Done),
    "failed" => Ok(StepOutcome::Failed),
    "unverified" => Ok(StepOutcome::Unverified),
    other => Err(corrupt("step outcome", other)),
  }
}
