//! Rebuild only historical facts: never substitute today's task, permissions or later transcript.
use orchester_core::{
  connection::stream::AgentEvent,
  permission::AgentPermissions,
  run::{PendingToolApproval, review::ReviewedTask},
  store::{ActivityStore, RunStore},
};
use rusqlite::{OptionalExtension, params};

use super::{
  ProjectDb,
  activity::EVENT_SCHEMA,
  tool_approvals::{COLUMNS, StoredApproval},
};
use crate::{
  StoreError,
  rows::{corrupt, decode_json},
};

pub struct RecordedDecisionReview {
  pub request: PendingToolApproval,
  pub permissions: Option<AgentPermissions>,
  pub step_name: Option<String>,
  pub task: Option<ReviewedTask>,
  pub messages: Vec<String>,
  pub previously_asked: bool,
}

impl ProjectDb {
  /// Each reviewed approval once, including pending/interrupted ones; retries use the last recorded review.
  pub fn recorded_decision_reviews(&self) -> Result<Vec<RecordedDecisionReview>, StoreError> {
    let mut query = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM tool_approvals WHERE id IN (SELECT approval_id FROM reviewer_usage) ORDER BY requested_at, id"
    ))?;
    let stored = query
      .query_map([], StoredApproval::read)?
      .collect::<Result<Vec<_>, _>>()?;
    stored
      .into_iter()
      .map(|row| self.recorded_decision_review(row.into_pending()?))
      .collect()
  }

  fn recorded_decision_review(&self, request: PendingToolApproval) -> Result<RecordedDecisionReview, StoreError> {
    let (at, verdict): (i64, String) = self.connection.query_row(
      "SELECT started_at, verdict FROM reviewer_usage WHERE approval_id = ?1 ORDER BY id DESC LIMIT 1",
      [&request.id],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let permissions = self
      .invocations(request.run)?
      .into_iter()
      .find(|invocation| invocation.id == request.invocation)
      .and_then(|invocation| invocation.permissions)
      .map(|launch| launch.permissions().clone());
    let run = self
      .run(request.run)
      .map_err(|error| corrupt("recorded review run", error))?;
    let step_name = run.as_ref().and_then(|run| {
      let execution = run.execution(request.execution)?;
      run
        .snapshot()
        .body()
        .step(execution.step())
        .map(|step| step.name.as_str().to_owned())
    });
    let task = self.recorded_review_task(&request)?;
    let messages = self.recorded_review_messages(&request, at)?;
    Ok(RecordedDecisionReview {
      request,
      permissions,
      step_name,
      task,
      messages,
      previously_asked: verdict != "approved",
    })
  }

  fn recorded_review_task(&self, request: &PendingToolApproval) -> Result<Option<ReviewedTask>, StoreError> {
    let row: Option<(String, String, String)> = self
      .connection
      .query_row(
        "SELECT title, brief, criteria FROM run_task_snapshots WHERE run_id = ?1",
        [request.run.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
      )
      .optional()?;
    row
      .map(|(title, brief, criteria)| {
        Ok(ReviewedTask {
          title,
          brief,
          criteria: decode_json("run task criteria", 1, &criteria)?,
        })
      })
      .transpose()
  }

  fn recorded_review_messages(&self, request: &PendingToolApproval, at: i64) -> Result<Vec<String>, StoreError> {
    let mut query = self.connection.prepare(
      "SELECT payload FROM agent_events WHERE run_id = ?1 AND step_execution_id = ?2 AND kind = 'message' \
       AND at <= ?3 ORDER BY sequence DESC LIMIT 3",
    )?;
    let payloads = query
      .query_map(
        params![request.run.to_string(), request.execution.to_string(), at],
        |row| row.get::<_, String>(0),
      )?
      .collect::<Result<Vec<_>, _>>()?;
    let mut messages = payloads
      .iter()
      .map(|payload| match decode_json("agent event", EVENT_SCHEMA, payload)? {
        AgentEvent::Message { text } => Ok(text),
        _ => Err(corrupt("recorded review message", "not a message")),
      })
      .collect::<Result<Vec<_>, StoreError>>()?;
    messages.reverse();
    Ok(messages)
  }
}
