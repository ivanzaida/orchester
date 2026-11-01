//! Decision providers share the reviewer ledger and budgets, without pretending to be agent connections.
use orchester_core::{
  id::RunId,
  resource::ModelId,
  run::review::{DecisionReviewRecord, ReviewEnd},
  time::Timestamp,
};
use rusqlite::params;

use super::{
  ProjectDb,
  reviewer_usage::{parse_verdict, verdict_text},
};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse, tokens_column},
};

impl ProjectDb {
  pub fn record_decision_reviewer_usage(&mut self, record: &DecisionReviewRecord) -> Result<(), StoreError> {
    let input = record
      .input_tokens
      .map(|n| tokens_column("decision input tokens", n))
      .transpose()?;
    let failure = match record.end {
      ReviewEnd::Failed(failure) => Some(encode_json("decision failure", 1, &failure)?),
      _ => None,
    };
    self.connection.execute(
      "INSERT INTO reviewer_usage (run_id, step_execution_id, invocation_id, approval_id, provider, model,
       started_at, ended_at, input_tokens, cached_input_tokens, output_tokens, verdict, connection_id,
       connection_name, decision_failure) VALUES (?1, ?2, ?3, ?4, 'decision', ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11, ?12, ?13)",
      params![record.run.to_string(), record.execution.to_string(), record.invocation.to_string(),
        record.approval, record.model.as_str(), record.started_at.unix_millis(), record.ended_at.unix_millis(),
        input, input.map(|_| 0), verdict_text(record.end), record.connection.to_string(), record.provider, failure],
    )?;
    Ok(())
  }

  pub fn decision_reviewer_usage(&self, run: RunId) -> Result<Vec<DecisionReviewRecord>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT run_id, step_execution_id, invocation_id, approval_id, model, started_at, ended_at,
       input_tokens, verdict, connection_id, connection_name, decision_failure FROM reviewer_usage
       WHERE run_id = ?1 AND provider = 'decision' ORDER BY ended_at, id",
    )?;
    let rows = statement
      .query_map([run.to_string()], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, String>(3)?,
          row.get::<_, String>(4)?,
          row.get::<_, i64>(5)?,
          row.get::<_, i64>(6)?,
          row.get::<_, Option<i64>>(7)?,
          row.get::<_, String>(8)?,
          row.get::<_, String>(9)?,
          row.get::<_, String>(10)?,
          row.get::<_, Option<String>>(11)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(
        |(run, execution, invocation, approval, model, start, end, input, verdict, connection, provider, failure)| {
          Ok(DecisionReviewRecord {
            run: parse("run id", &run)?,
            execution: parse("execution id", &execution)?,
            invocation: parse("invocation id", &invocation)?,
            approval,
            model: ModelId::new(&model).map_err(|e| corrupt("decision model", e))?,
            started_at: Timestamp::from_unix_millis(start),
            ended_at: Timestamp::from_unix_millis(end),
            input_tokens: input
              .map(|n| u64::try_from(n).map_err(|e| corrupt("decision tokens", e)))
              .transpose()?,
            end: match failure {
              Some(failure) => ReviewEnd::Failed(decode_json("decision failure", 1, &failure)?),
              None => parse_verdict(&verdict)?,
            },
            connection: parse("decision connection id", &connection)?,
            provider,
          })
        },
      )
      .collect()
  }
}
