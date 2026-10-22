//! What auto mode's reviewer spent (`reviewer_usage`): one record per review that asked a model, written
//! by the runner, read for the run's and the step's budgets, the daily budget and the Usage dashboard.

use std::sync::Arc;

use orchester_core::{
  id::{AgentAccountId, RunId},
  resource::ModelId,
  run::{
    TokenUsage,
    review::{ReviewEnd, ReviewFailure, ReviewRecord},
  },
  time::Timestamp,
  tokens::TokensUsed,
  usage::ReviewUsage,
};
use rusqlite::{Connection, Row, params};

use super::{
  ProjectDb,
  connections::{parse_provider, provider_text},
};
use crate::{
  StoreError,
  rows::{corrupt, parse, tokens_column},
};

const SELECT: &str = "SELECT run_id, step_execution_id, invocation_id, approval_id, provider, model, started_at, \
                      ended_at, input_tokens, cached_input_tokens, output_tokens, verdict, account_id \
                      FROM reviewer_usage";

impl ProjectDb {
  /// Stores what one review cost.
  pub fn record_reviewer_usage(&mut self, record: &ReviewRecord) -> Result<(), StoreError> {
    let tokens = |count: fn(TokenUsage) -> u64| {
      record
        .usage
        .map(|usage| tokens_column("reviewer tokens", count(usage)))
        .transpose()
    };
    self.connection.execute(
      "INSERT INTO reviewer_usage (run_id, step_execution_id, invocation_id, approval_id, provider, model, started_at, \
       ended_at, input_tokens, cached_input_tokens, output_tokens, verdict, account_id) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
      params![
        record.run.to_string(),
        record.execution.to_string(),
        record.invocation.to_string(),
        record.approval,
        provider_text(record.provider),
        record.model.as_str(),
        record.started_at.unix_millis(),
        record.ended_at.unix_millis(),
        tokens(|usage| usage.input)?,
        tokens(|usage| usage.cached_input)?,
        tokens(|usage| usage.output)?,
        verdict_text(record.end),
        record.account.map(|account| account.to_string()),
      ],
    )?;
    Ok(())
  }

  /// The reviews of `run`'s requests, in the order they ended.
  pub fn reviewer_usage(&self, run: RunId) -> Result<Vec<ReviewRecord>, StoreError> {
    let mut statement = self
      .connection
      .prepare(&format!("{SELECT} WHERE run_id = ?1 ORDER BY ended_at, id"))?;
    let rows = statement
      .query_map([run.to_string()], RecordRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(RecordRow::into_record).collect()
  }

  /// The reviews that ended from `since` on and reported what they spent, for the Usage dashboard.
  pub(super) fn reviews_since(&self, since: Timestamp) -> Result<Vec<ReviewUsage>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "{SELECT} WHERE ended_at >= ?1 AND input_tokens IS NOT NULL ORDER BY ended_at, id"
    ))?;
    let rows = statement
      .query_map([since.unix_millis()], RecordRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    let mut reviews = Vec::new();
    for row in rows {
      let record = row.into_record()?;
      if let Some(usage) = record.usage {
        reviews.push(ReviewUsage {
          at: record.ended_at,
          invocation: record.invocation,
          cli: record.provider,
          account: record.account,
          model: Arc::from(record.model.as_str()),
          usage,
        });
      }
    }
    Ok(reviews)
  }
}

/// Tokens that counted toward budgets in the reviews that ended from `since` on; unknown usage counts
/// nothing.
pub(super) fn counted_since(connection: &Connection, since: Timestamp) -> Result<TokensUsed, StoreError> {
  let mut statement = connection.prepare(
    "SELECT input_tokens, cached_input_tokens, output_tokens FROM reviewer_usage \
     WHERE ended_at >= ?1 AND input_tokens IS NOT NULL",
  )?;
  let rows = statement
    .query_map([since.unix_millis()], |row| {
      Ok([row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?])
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|tokens| usage(tokens).map(TokenUsage::counted_tokens))
    .sum()
}

fn usage([input, cached_input, output]: [i64; 3]) -> Result<TokenUsage, StoreError> {
  let count = |value: i64| u64::try_from(value).map_err(|_| corrupt("reviewer tokens", value));
  Ok(TokenUsage {
    input: count(input)?,
    cached_input: count(cached_input)?,
    output: count(output)?,
  })
}

const fn verdict_text(end: ReviewEnd) -> &'static str {
  match end {
    ReviewEnd::Approved => "approved",
    ReviewEnd::Asked => "asked",
    ReviewEnd::Failed(ReviewFailure::TimedOut) => "timed_out",
    ReviewEnd::Failed(ReviewFailure::Unreadable) => "unreadable",
    ReviewEnd::Failed(ReviewFailure::CouldNotRun) => "could_not_run",
    ReviewEnd::Failed(ReviewFailure::NoReviewer) => "no_reviewer",
  }
}

fn parse_verdict(text: &str) -> Result<ReviewEnd, StoreError> {
  Ok(match text {
    "approved" => ReviewEnd::Approved,
    "asked" => ReviewEnd::Asked,
    "timed_out" => ReviewEnd::Failed(ReviewFailure::TimedOut),
    "unreadable" => ReviewEnd::Failed(ReviewFailure::Unreadable),
    "could_not_run" => ReviewEnd::Failed(ReviewFailure::CouldNotRun),
    "no_reviewer" => ReviewEnd::Failed(ReviewFailure::NoReviewer),
    other => return Err(corrupt("review verdict", other)),
  })
}

struct RecordRow {
  ids: [String; 4],
  provider: String,
  model: String,
  started_at: i64,
  ended_at: i64,
  tokens: [Option<i64>; 3],
  verdict: String,
  account: Option<String>,
}

impl RecordRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      ids: [row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?],
      provider: row.get(4)?,
      model: row.get(5)?,
      started_at: row.get(6)?,
      ended_at: row.get(7)?,
      tokens: [row.get(8)?, row.get(9)?, row.get(10)?],
      verdict: row.get(11)?,
      account: row.get(12)?,
    })
  }

  fn into_record(self) -> Result<ReviewRecord, StoreError> {
    let [run, execution, invocation, approval] = self.ids;
    let usage = match self.tokens {
      [Some(input), Some(cached_input), Some(output)] => Some(usage([input, cached_input, output])?),
      [None, None, None] => None,
      partly => return Err(corrupt("reviewer tokens", format!("{partly:?}"))),
    };
    Ok(ReviewRecord {
      run: parse("run id", &run)?,
      execution: parse("step execution id", &execution)?,
      invocation: parse("invocation id", &invocation)?,
      approval,
      provider: parse_provider(&self.provider)?,
      account: self
        .account
        .map(|account| parse::<AgentAccountId>("account id", &account))
        .transpose()?,
      model: ModelId::new(&self.model).map_err(|error| corrupt("reviewer model", error))?,
      started_at: Timestamp::from_unix_millis(self.started_at),
      ended_at: Timestamp::from_unix_millis(self.ended_at),
      usage,
      end: parse_verdict(&self.verdict)?,
    })
  }
}
