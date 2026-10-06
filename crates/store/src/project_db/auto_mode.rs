//! Auto mode (`orchester_core::run::review`): the project's setting, each run's own choice, what auto mode
//! did with a request (`tool_approvals.review`), and the agent's last messages its reviewer reads.

use orchester_core::{
  connection::stream::AgentEvent,
  id::{ConnectionId, RunId, StepExecutionId},
  resource::ModelId,
  run::review::{AutoModeSettings, DEFAULT_REVIEW_MODEL, Review, ReviewerChoice},
};
use rusqlite::{OptionalExtension, params};

use super::{ProjectDb, activity::EVENT_SCHEMA};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json},
};

const REVIEW: &str = "tool approval review";
const REVIEW_SCHEMA: u32 = 1;

impl ProjectDb {
  /// Whether new runs use auto mode, and the reviewer a person chose; off, with the default reviewer,
  /// until a person changes them.
  pub fn auto_mode_settings(&self) -> Result<AutoModeSettings, StoreError> {
    let stored: Option<(bool, String, Option<String>, Option<String>)> = self
      .connection
      .query_row(
        "SELECT new_runs, model, connection, effort FROM project_auto_mode WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
      )
      .optional()?;
    let Some((new_runs, model, connection, effort)) = stored else {
      return Ok(AutoModeSettings::default());
    };
    let reviewer = match connection {
      None => None,
      Some(connection) => Some(ReviewerChoice {
        connection: connection
          .parse::<ConnectionId>()
          .map_err(|_| corrupt("auto mode reviewer connection", connection))?,
        model: ModelId::new(&model).map_err(|_| corrupt("auto mode model", model))?,
        effort,
      }),
    };
    Ok(AutoModeSettings { new_runs, reviewer })
  }

  pub fn save_auto_mode_settings(&mut self, settings: &AutoModeSettings) -> Result<(), StoreError> {
    let reviewer = settings.reviewer.as_ref();
    self.connection.execute(
      "INSERT INTO project_auto_mode (id, new_runs, model, connection, effort) VALUES (1, ?1, ?2, ?3, ?4)        ON CONFLICT (id) DO UPDATE SET new_runs = excluded.new_runs, model = excluded.model,        connection = excluded.connection, effort = excluded.effort",
      params![
        settings.new_runs,
        reviewer.map_or(DEFAULT_REVIEW_MODEL, |choice| choice.model.as_str()),
        reviewer.map(|choice| choice.connection.to_string()),
        reviewer.and_then(|choice| choice.effort.as_deref()),
      ],
    )?;
    Ok(())
  }

  /// Whether `run` uses auto mode.
  pub fn run_auto_mode(&self, run: RunId) -> Result<bool, StoreError> {
    self
      .connection
      .query_row("SELECT auto_mode FROM runs WHERE id = ?1", [run.to_string()], |row| {
        row.get(0)
      })
      .optional()?
      .ok_or_else(|| corrupt("run", format!("{run} is not stored")))
  }

  pub fn set_run_auto_mode(&mut self, run: RunId, on: bool) -> Result<(), StoreError> {
    let changed = self.connection.execute(
      "UPDATE runs SET auto_mode = ?2 WHERE id = ?1",
      params![run.to_string(), on],
    )?;
    if changed == 0 {
      return Err(corrupt("run", format!("{run} is not stored")));
    }
    Ok(())
  }

  /// Records what auto mode did with request `id` of `run`, while it waits.
  pub fn record_review(&mut self, run: RunId, id: &str, review: &Review) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE tool_approvals SET review = ?3 WHERE run_id = ?1 AND id = ?2 AND state = 'pending'",
      params![run.to_string(), id, encode_json(REVIEW, REVIEW_SCHEMA, review)?],
    )?;
    Ok(())
  }

  /// What auto mode did with request `id` of `run`; `None` when it did not look at it.
  pub fn tool_approval_review(&self, run: RunId, id: &str) -> Result<Option<Review>, StoreError> {
    let stored: Option<Option<String>> = self
      .connection
      .query_row(
        "SELECT review FROM tool_approvals WHERE run_id = ?1 AND id = ?2",
        params![run.to_string(), id],
        |row| row.get(0),
      )
      .optional()?;
    stored
      .flatten()
      .map(|text| decode_json(REVIEW, REVIEW_SCHEMA, &text))
      .transpose()
  }

  /// The last `count` messages the agent of `execution` wrote in `run`, oldest first.
  pub fn recent_agent_messages(
    &self,
    run: RunId,
    execution: StepExecutionId,
    count: usize,
  ) -> Result<Vec<String>, StoreError> {
    let limit = i64::try_from(count).map_err(|_| corrupt("message count", count))?;
    let mut query = self.connection.prepare(
      "SELECT payload FROM agent_events WHERE run_id = ?1 AND step_execution_id = ?2 AND kind = 'message' \
       ORDER BY sequence DESC LIMIT ?3",
    )?;
    let payloads = query
      .query_map(params![run.to_string(), execution.to_string(), limit], |row| {
        row.get::<_, String>(0)
      })?
      .collect::<Result<Vec<_>, _>>()?;
    let mut messages = payloads
      .iter()
      .map(|payload| match decode_json("agent event", EVENT_SCHEMA, payload)? {
        AgentEvent::Message { text } => Ok(text),
        other => Err(corrupt(
          "agent event",
          format!("kind `message` holds a `{}` payload", other.name()),
        )),
      })
      .collect::<Result<Vec<_>, StoreError>>()?;
    messages.reverse();
    Ok(messages)
  }
}

/// `review` as the column stores it, for a decided request read with others.
pub(super) fn decode_review(stored: Option<String>) -> Result<Option<Review>, StoreError> {
  stored.map(|text| decode_json(REVIEW, REVIEW_SCHEMA, &text)).transpose()
}
