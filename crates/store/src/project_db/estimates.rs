//! Story points (`task_estimates`) and what the estimator spent (`estimate_usage`). A person's points are never
//! replaced by automation's: the write that would do so changes nothing, in the same statement.

use orchester_core::{
  connection::AgentProvider,
  id::TaskId,
  resource::ModelId,
  run::{TokenUsage, review::ReviewFailure},
  task::estimate::{EstimateBasis, EstimateRecord, EstimateSource, StoryEstimate, StoryPoints},
  time::Timestamp,
  tokens::TokensUsed,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{ProjectDb, connections::provider_text, tasks::load_task};
use crate::{
  StoreError,
  rows::{corrupt, tokens_column},
};

/// What became of automation's points ([`ProjectDb::set_automation_points`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutomationPoints {
  /// They are the task's points now.
  Stored,
  /// A person set the task's points: they stay.
  PersonsValue,
  /// The task's brief or criteria changed since the estimate was made from them: it was not kept.
  Outdated,
}

impl ProjectDb {
  /// Whether the project lets Orchester estimate story points (*Let Orchester estimate story points*); `None` until a
  /// person sets it for the project, when the application-wide setting it moved from holds.
  pub fn estimates_story_points(&self) -> Result<Option<bool>, StoreError> {
    Ok(
      self
        .connection
        .query_row("SELECT allowed FROM project_estimates WHERE id = 1", [], |row| {
          row.get(0)
        })
        .optional()?,
    )
  }

  pub fn set_estimates_story_points(&mut self, allowed: bool) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO project_estimates (id, allowed) VALUES (1, ?1) ON CONFLICT (id) DO UPDATE SET allowed = ?1",
      params![allowed],
    )?;
    Ok(())
  }

  /// The story points of `task`; `None` while it has none.
  pub fn story_estimate(&self, task: TaskId) -> Result<Option<StoryEstimate>, StoreError> {
    load(&self.connection, task)
  }

  /// A person's points for `task`, or, with `None`, none of a person's: automation may estimate it again. A value
  /// automation set stays when the person sets none.
  pub fn set_person_points(
    &mut self,
    task: TaskId,
    points: Option<StoryPoints>,
    at: Timestamp,
  ) -> Result<(), StoreError> {
    match points {
      Some(points) => self.connection.execute(
        "INSERT INTO task_estimates (task_id, points, source, reason, basis, set_at) \
         VALUES (?1, ?2, 'person', NULL, NULL, ?3) \
         ON CONFLICT (task_id) DO UPDATE SET points = ?2, source = 'person', reason = NULL, basis = NULL, set_at = ?3",
        params![task.to_string(), points.get(), at.unix_millis()],
      )?,
      None => self.connection.execute(
        "DELETE FROM task_estimates WHERE task_id = ?1 AND source = 'person'",
        [task.to_string()],
      )?,
    };
    Ok(())
  }

  /// Automation's `estimate` of `task`, unless a person set its points or the task's brief and criteria are no
  /// longer those the estimate was made from: an estimate that took a while never replaces one of a later brief.
  pub fn set_automation_points(
    &mut self,
    task: TaskId,
    estimate: &StoryEstimate,
  ) -> Result<AutomationPoints, StoreError> {
    let transaction = self
      .connection
      .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current = load_task(&transaction, self.project.key(), task)?;
    if estimate
      .basis()
      .is_some_and(|basis| *basis != EstimateBasis::of(current.details()))
    {
      return Ok(AutomationPoints::Outdated);
    }
    let stored = transaction.execute(
      "INSERT INTO task_estimates (task_id, points, source, reason, basis, set_at) \
       VALUES (?1, ?2, 'automation', ?3, ?4, ?5) \
       ON CONFLICT (task_id) DO UPDATE SET points = ?2, source = 'automation', reason = ?3, basis = ?4, set_at = ?5 \
       WHERE task_estimates.source = 'automation'",
      params![
        task.to_string(),
        estimate.points().get(),
        estimate.reason(),
        estimate.basis().map(EstimateBasis::as_str),
        estimate.at().unix_millis(),
      ],
    )?;
    transaction.commit()?;
    Ok(if stored == 1 {
      AutomationPoints::Stored
    } else {
      AutomationPoints::PersonsValue
    })
  }

  /// Keeps what one request to the estimator spent.
  pub fn record_estimate_usage(&mut self, record: &EstimateRecord) -> Result<(), StoreError> {
    let tokens = |count: fn(TokenUsage) -> u64| {
      record
        .usage
        .map(|usage| tokens_column("estimate tokens", count(usage)))
        .transpose()
    };
    self.connection.execute(
      "INSERT INTO estimate_usage (task_id, basis, provider, model, started_at, ended_at, input_tokens, \
       cached_input_tokens, output_tokens, failure) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
      params![
        record.task.to_string(),
        record.basis.as_str(),
        provider_text(record.provider),
        record.model.as_str(),
        record.started_at.unix_millis(),
        record.ended_at.unix_millis(),
        tokens(|usage| usage.input)?,
        tokens(|usage| usage.cached_input)?,
        tokens(|usage| usage.output)?,
        record.failure.map(failure_text),
      ],
    )?;
    Ok(())
  }

  /// Whether the estimator was asked about `task` from `basis` already.
  pub fn estimate_asked(&self, task: TaskId, basis: &EstimateBasis) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      "SELECT EXISTS(SELECT 1 FROM estimate_usage WHERE task_id = ?1 AND basis = ?2)",
      params![task.to_string(), basis.as_str()],
      |row| row.get(0),
    )?)
  }

  /// Why the last request to the estimator on `provider` with `model` that asked a model gave no points; `None` when
  /// it gave them, or none was recorded.
  pub fn last_estimate_failure(
    &self,
    (provider, model): (AgentProvider, &ModelId),
  ) -> Result<Option<ReviewFailure>, StoreError> {
    let failure: Option<Option<String>> = self
      .connection
      .query_row(
        "SELECT failure FROM estimate_usage WHERE provider = ?1 AND model = ?2 ORDER BY ended_at DESC, id DESC LIMIT 1",
        params![provider_text(provider), model.as_str()],
        |row| row.get(0),
      )
      .optional()?;
    failure.flatten().as_deref().map(parse_failure).transpose()
  }

  /// What the estimator's requests that ended from `start` until before `end` reported they spent, summed, and how
  /// many asked a model.
  pub fn estimates_between(&self, start: Timestamp, end: Timestamp) -> Result<(TokenUsage, u64), StoreError> {
    let rows = usage_rows(&self.connection, start, Some(end))?;
    let count = u64::try_from(rows.len()).map_err(|_| StoreError::Unrepresentable("estimate count"))?;
    let total = rows
      .into_iter()
      .flatten()
      .map(usage)
      .sum::<Result<TokenUsage, StoreError>>()?;
    Ok((total, count))
  }
}

/// Tokens that counted toward budgets in the estimator's requests that ended from `since` on; unknown usage counts
/// nothing.
pub(super) fn counted_since(connection: &Connection, since: Timestamp) -> Result<TokensUsed, StoreError> {
  usage_rows(connection, since, None)?
    .into_iter()
    .flatten()
    .map(|tokens| usage(tokens).map(TokenUsage::counted_tokens))
    .sum()
}

/// The tokens of each request that ended from `start` on, before `end` when it is given; `None` where they could not
/// be read.
fn usage_rows(
  connection: &Connection,
  start: Timestamp,
  end: Option<Timestamp>,
) -> Result<Vec<Option<[i64; 3]>>, StoreError> {
  let mut statement = connection.prepare(
    "SELECT input_tokens, cached_input_tokens, output_tokens FROM estimate_usage \
     WHERE ended_at >= ?1 AND (?2 IS NULL OR ended_at < ?2)",
  )?;
  let rows = statement
    .query_map(params![start.unix_millis(), end.map(Timestamp::unix_millis)], |row| {
      let tokens: [Option<i64>; 3] = [row.get(0)?, row.get(1)?, row.get(2)?];
      Ok(tokens)
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|tokens| match tokens {
      [Some(input), Some(cached_input), Some(output)] => Ok(Some([input, cached_input, output])),
      [None, None, None] => Ok(None),
      partly => Err(corrupt("estimate tokens", format!("{partly:?}"))),
    })
    .collect()
}

fn usage([input, cached_input, output]: [i64; 3]) -> Result<TokenUsage, StoreError> {
  let count = |value: i64| u64::try_from(value).map_err(|_| corrupt("estimate tokens", value));
  Ok(TokenUsage {
    input: count(input)?,
    cached_input: count(cached_input)?,
    output: count(output)?,
  })
}

/// A `task_estimates` row: points, source, reason, basis and when they were set.
type EstimateRow = (i64, String, Option<String>, Option<String>, i64);

/// The story points of `task` read in `connection`.
pub(super) fn load(connection: &Connection, task: TaskId) -> Result<Option<StoryEstimate>, StoreError> {
  let row: Option<EstimateRow> = connection
    .query_row(
      "SELECT points, source, reason, basis, set_at FROM task_estimates WHERE task_id = ?1",
      [task.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )
    .optional()?;
  row
    .map(|(points, source, reason, basis, at)| {
      Ok(StoryEstimate::restored(
        (points_value(points)?, parse_source(&source)?),
        reason,
        basis.map(EstimateBasis::restored),
        Timestamp::from_unix_millis(at),
      ))
    })
    .transpose()
}

pub(super) fn points_value(points: i64) -> Result<StoryPoints, StoreError> {
  u32::try_from(points)
    .ok()
    .and_then(|points| StoryPoints::new(points).ok())
    .ok_or_else(|| corrupt("story points", points))
}

pub(super) const fn source_text(source: EstimateSource) -> &'static str {
  match source {
    EstimateSource::Person => "person",
    EstimateSource::Automation => "automation",
  }
}

pub(super) fn parse_source(text: &str) -> Result<EstimateSource, StoreError> {
  match text {
    "person" => Ok(EstimateSource::Person),
    "automation" => Ok(EstimateSource::Automation),
    other => Err(corrupt("story points source", other)),
  }
}

const fn failure_text(failure: ReviewFailure) -> &'static str {
  match failure {
    ReviewFailure::TimedOut => "timed_out",
    ReviewFailure::Unreadable => "unreadable",
    ReviewFailure::CouldNotRun => "could_not_run",
    ReviewFailure::NoReviewer => "no_reviewer",
  }
}

fn parse_failure(text: &str) -> Result<ReviewFailure, StoreError> {
  Ok(match text {
    "timed_out" => ReviewFailure::TimedOut,
    "unreadable" => ReviewFailure::Unreadable,
    "could_not_run" => ReviewFailure::CouldNotRun,
    "no_reviewer" => ReviewFailure::NoReviewer,
    other => return Err(corrupt("estimate failure", other)),
  })
}
