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
  rows::{corrupt, parse},
};

pub(super) fn save(connection: &Connection, output: &StepOutput) -> Result<(), StoreError> {
  connection.execute(
    "INSERT INTO step_outputs (step_execution_id, run_id, step_key, outcome, text, created_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT (step_execution_id) DO NOTHING",
    params![
      output.execution.to_string(),
      output.run.to_string(),
      output.step.as_str(),
      output.outcome.map(outcome_text),
      output.text,
      output.created_at.unix_millis(),
    ],
  )?;
  Ok(())
}

pub(super) fn of_run(connection: &Connection, run: RunId) -> Result<Vec<StepOutput>, StoreError> {
  let mut statement = connection.prepare(
    "SELECT step_execution_id, step_key, outcome, text, created_at FROM step_outputs WHERE run_id = ?1 \
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
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(execution, step, outcome, text, created_at)| {
      Ok(StepOutput {
        run,
        execution: parse("step execution id", &execution)?,
        step: orchester_core::resource::StepKey::new(&step).map_err(|error| corrupt("step key", error))?,
        outcome: outcome.as_deref().map(parse_outcome).transpose()?,
        text,
        created_at: Timestamp::from_unix_millis(created_at),
      })
    })
    .collect()
}

const fn outcome_text(outcome: StepOutcome) -> &'static str {
  match outcome {
    StepOutcome::Done => "done",
    StepOutcome::Failed => "failed",
  }
}

fn parse_outcome(value: &str) -> Result<StepOutcome, StoreError> {
  match value {
    "done" => Ok(StepOutcome::Done),
    "failed" => Ok(StepOutcome::Failed),
    other => Err(corrupt("step outcome", other)),
  }
}
