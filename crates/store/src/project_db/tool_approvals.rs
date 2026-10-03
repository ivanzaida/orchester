//! Durable display record for each live Codex tool approval. Only the worker that owns the
//! JSON-RPC request can act on a decision; an interrupted worker marks it interrupted.

use orchester_core::{
  id::{InvocationId, RunId},
  run::{CommandShell, DecidedBy, DecidedToolApproval, NativeCommandAsk, PendingToolApproval, ToolApprovalDecision},
  time::Timestamp,
};
use rusqlite::params;

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// The columns [`StoredApproval::read`] reads, in its order.
const COLUMNS: &str = "id, run_id, step_execution_id, invocation_id, agent, server, action, target, requested_at, \
                       command, scope, native_shell, native_time_limit";

impl ProjectDb {
  pub fn pending_tool_approvals(&self, run: RunId) -> Result<Vec<PendingToolApproval>, StoreError> {
    self.pending_tool_approvals_for(Some(run))
  }

  /// All live decisions for a project shell, oldest first across runs.
  pub fn pending_tool_approvals_all(&self) -> Result<Vec<PendingToolApproval>, StoreError> {
    self.pending_tool_approvals_for(None)
  }

  fn pending_tool_approvals_for(&self, run: Option<RunId>) -> Result<Vec<PendingToolApproval>, StoreError> {
    let mut query = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM tool_approvals WHERE state = 'pending' AND (?1 IS NULL OR run_id = ?1) \
       ORDER BY requested_at, id"
    ))?;
    let rows = query
      .query_map([run.map(|id| id.to_string())], StoredApproval::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(StoredApproval::into_pending).collect()
  }

  /// `run`'s requests that are no longer pending, oldest first: how each was decided, or that it was
  /// interrupted. One whose decision is still on its way to the agent reads as that decision.
  pub fn decided_tool_approvals(&self, run: RunId) -> Result<Vec<DecidedToolApproval>, StoreError> {
    let mut query = self.connection.prepare(&format!(
      "SELECT {COLUMNS}, state, decided_at FROM tool_approvals WHERE state != 'pending' AND run_id = ?1 \
       ORDER BY requested_at, id"
    ))?;
    let rows = query
      .query_map([run.to_string()], |row| {
        Ok((
          StoredApproval::read(row)?,
          row.get::<_, String>(13)?,
          row.get::<_, i64>(14)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(stored, state, decided_at)| {
        let decision = match state.as_str() {
          "approved" | "dispatching_approval" => Some(ToolApprovalDecision::Approved),
          "denied" | "dispatching_denial" => Some(ToolApprovalDecision::Denied),
          "interrupted" => None,
          other => return Err(corrupt("tool approval state", other)),
        };
        Ok(DecidedToolApproval {
          request: stored.into_pending()?,
          decision,
          decided_at: Timestamp::from_unix_millis(decided_at),
        })
      })
      .collect()
  }

  /// Creates one pending request. A repeated JSON-RPC request ID is refused rather than
  /// displaying a second prompt that might approve a different call.
  pub fn request_tool_approval(&mut self, approval: &PendingToolApproval) -> Result<(), StoreError> {
    let native = approval.native.as_ref();
    self.connection.execute(
      "INSERT INTO tool_approvals (id, run_id, step_execution_id, invocation_id, agent, server, action, \
       target, requested_at, command, scope, native_shell, native_time_limit) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
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
        native.and_then(|native| native.shell).map(CommandShell::as_str),
        native.map(|native| native.time_limit_seconds),
      ],
    )?;
    Ok(())
  }

  /// Atomically removes the request from pending before its answer is sent, recording who answered.
  pub fn claim_tool_approval(
    &mut self,
    run: RunId,
    id: &str,
    decided_by: DecidedBy,
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    let state = if decided_by.approves() {
      "dispatching_approval"
    } else {
      "dispatching_denial"
    };
    let changed = self.connection.execute(
      "UPDATE tool_approvals SET state = ?3, decided_at = ?4, decided_by = ?5 \
       WHERE run_id = ?1 AND id = ?2 AND state = 'pending'",
      params![run.to_string(), id, state, at.unix_millis(), decided_by.as_str()],
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

  /// Request `id` of `run` waits for an agent that can no longer receive an answer: its invocation
  /// ended, its runner is gone, or no live agent ever held it. It ends interrupted, which reads as
  /// ended without an answer; whether it was still waiting.
  pub fn end_waiting_tool_approval(&mut self, run: RunId, id: &str) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE tool_approvals SET state = 'interrupted', decided_at = ?3 \
       WHERE run_id = ?1 AND id = ?2 AND state = 'pending'",
      params![run.to_string(), id, Timestamp::now().unix_millis()],
    )?;
    Ok(changed == 1)
  }

  /// Whether request `id` of `run` was answered, by a person or by Orchester: its answer is recorded,
  /// delivered or on its way. One that ended without an answer, or is unknown, was not.
  pub fn tool_approval_answered(&self, run: RunId, id: &str) -> Result<bool, StoreError> {
    let count: i64 = self.connection.query_row(
      "SELECT COUNT(*) FROM tool_approvals WHERE run_id = ?1 AND id = ?2 \
       AND state IN ('dispatching_approval', 'dispatching_denial', 'approved', 'denied')",
      params![run.to_string(), id],
      |row| row.get(0),
    )?;
    Ok(count > 0)
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

/// A `tool_approvals` row as read, before its ids and native details are checked.
struct StoredApproval {
  id: String,
  run: String,
  execution: String,
  invocation: String,
  agent: String,
  server: String,
  action: String,
  target: Option<String>,
  requested_at: i64,
  command: bool,
  scope: Option<String>,
  native_shell: Option<String>,
  native_time_limit: Option<u32>,
}

impl StoredApproval {
  fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      id: row.get(0)?,
      run: row.get(1)?,
      execution: row.get(2)?,
      invocation: row.get(3)?,
      agent: row.get(4)?,
      server: row.get(5)?,
      action: row.get(6)?,
      target: row.get(7)?,
      requested_at: row.get(8)?,
      command: row.get(9)?,
      scope: row.get(10)?,
      native_shell: row.get(11)?,
      native_time_limit: row.get(12)?,
    })
  }

  fn into_pending(self) -> Result<PendingToolApproval, StoreError> {
    let shell = self
      .native_shell
      .as_deref()
      .map(|shell| parse::<CommandShell>("approval shell", shell))
      .transpose()?;
    let native = self.native_time_limit.map(|time_limit_seconds| NativeCommandAsk {
      shell,
      time_limit_seconds,
    });
    Ok(PendingToolApproval {
      id: self.id,
      run: parse("approval run id", &self.run)?,
      execution: parse("approval execution id", &self.execution)?,
      invocation: parse("approval invocation id", &self.invocation)?,
      agent: self.agent,
      command: self.command,
      scope: self.scope,
      server: self.server,
      action: self.action,
      target: self.target,
      requested_at: Timestamp::from_unix_millis(self.requested_at),
      native,
    })
  }
}
