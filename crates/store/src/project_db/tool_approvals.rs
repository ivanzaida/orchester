//! Durable display record for each live Codex tool approval. Only the worker that owns the
//! JSON-RPC request can act on a decision; an interrupted worker marks it interrupted.

use orchester_core::{
  id::{InvocationId, RunId},
  run::{PendingToolApproval, ToolApprovalDecision},
  time::Timestamp,
};
use rusqlite::params;

use super::ProjectDb;
use crate::{StoreError, rows::parse};

impl ProjectDb {
  pub fn pending_tool_approvals(&self, run: RunId) -> Result<Vec<PendingToolApproval>, StoreError> {
    self.pending_tool_approvals_for(Some(run))
  }

  /// All live decisions for a project shell, oldest first across runs.
  pub fn pending_tool_approvals_all(&self) -> Result<Vec<PendingToolApproval>, StoreError> {
    self.pending_tool_approvals_for(None)
  }

  fn pending_tool_approvals_for(&self, run: Option<RunId>) -> Result<Vec<PendingToolApproval>, StoreError> {
    let mut query = self.connection.prepare(
      "SELECT id, run_id, step_execution_id, invocation_id, agent, server, action, target, requested_at, command, scope \
       FROM tool_approvals WHERE state = 'pending' AND (?1 IS NULL OR run_id = ?1) ORDER BY requested_at, id",
    )?;
    let rows = query
      .query_map([run.map(|id| id.to_string())], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, String>(2)?,
          row.get::<_, String>(3)?,
          row.get::<_, String>(4)?,
          row.get::<_, String>(5)?,
          row.get::<_, String>(6)?,
          row.get::<_, Option<String>>(7)?,
          row.get::<_, i64>(8)?,
          row.get::<_, bool>(9)?,
          row.get::<_, Option<String>>(10)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(
        |(id, run, execution, invocation, agent, server, action, target, requested_at, command, scope)| {
          Ok(PendingToolApproval {
            id,
            run: parse("approval run id", &run)?,
            execution: parse("approval execution id", &execution)?,
            invocation: parse("approval invocation id", &invocation)?,
            agent,
            command,
            scope,
            server,
            action,
            target,
            requested_at: Timestamp::from_unix_millis(requested_at),
          })
        },
      )
      .collect()
  }

  /// Creates one pending request. A repeated JSON-RPC request ID is refused rather than
  /// displaying a second prompt that might approve a different call.
  pub fn request_tool_approval(&mut self, approval: &PendingToolApproval) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO tool_approvals (id, run_id, step_execution_id, invocation_id, agent, server, action, \
       target, requested_at, command, scope) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
      params![
        approval.id,
        approval.run.to_string(),
        approval.execution.to_string(),
        approval.invocation.to_string(),
        approval.agent,
        approval.server,
        approval.action,
        approval.target,
        approval.requested_at.unix_millis(),
        approval.command,
        approval.scope,
      ],
    )?;
    Ok(())
  }

  /// Atomically removes the request from pending before sending its JSON-RPC response.
  pub fn claim_tool_approval(
    &mut self,
    run: RunId,
    id: &str,
    approve: bool,
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    let state = if approve {
      "dispatching_approval"
    } else {
      "dispatching_denial"
    };
    let changed = self.connection.execute(
      "UPDATE tool_approvals SET state = ?3, decided_at = ?4 \
       WHERE run_id = ?1 AND id = ?2 AND state = 'pending'",
      params![run.to_string(), id, state, at.unix_millis()],
    )?;
    Ok(changed == 1)
  }

  /// Records that the claimed response reached the child process's stdin writer.
  pub fn delivered_tool_approval(
    &mut self,
    run: RunId,
    id: &str,
    decision: ToolApprovalDecision,
  ) -> Result<bool, StoreError> {
    let claimed = if decision == ToolApprovalDecision::Approved {
      "dispatching_approval"
    } else {
      "dispatching_denial"
    };
    let changed = self.connection.execute(
      "UPDATE tool_approvals SET state = ?3 WHERE run_id = ?1 AND id = ?2 AND state = ?4",
      params![run.to_string(), id, decision.as_str(), claimed],
    )?;
    Ok(changed == 1)
  }

  /// The worker process is gone. A stale modal must never approve a restarted call.
  pub fn interrupt_tool_approvals(&mut self, run: RunId, invocation: InvocationId) -> Result<bool, StoreError> {
    // A delivered approval may have run without reporting completion before the process died.
    // Keep its immutable decision, but require a person to inspect the run before resuming it.
    let uncertain = self.connection.execute(
      "INSERT OR IGNORE INTO approval_interruptions (invocation_id) \
       SELECT invocation_id FROM tool_approvals WHERE run_id = ?1 AND invocation_id = ?2 AND state = 'approved'",
      params![run.to_string(), invocation.to_string()],
    )?;
    let changed = self.connection.execute(
      "UPDATE tool_approvals SET state = 'interrupted', decided_at = COALESCE(decided_at, ?3) \
       WHERE run_id = ?1 AND invocation_id = ?2 \
       AND state IN ('pending', 'dispatching_approval', 'dispatching_denial')",
      params![run.to_string(), invocation.to_string(), Timestamp::now().unix_millis()],
    )?;
    Ok(changed > 0 || uncertain > 0)
  }

  pub fn has_interrupted_tool_approval(&self, run: RunId) -> Result<bool, StoreError> {
    let count: i64 = self.connection.query_row(
      "SELECT COUNT(*) FROM tool_approvals WHERE run_id = ?1 AND (state = 'interrupted' OR \
       invocation_id IN (SELECT invocation_id FROM approval_interruptions))",
      [run.to_string()],
      |row| row.get(0),
    )?;
    Ok(count > 0)
  }
}
