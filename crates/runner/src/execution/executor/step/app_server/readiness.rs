//! Bounded readiness of required MCP connections. Targeted thread-scoped status reads reuse the
//! thread's runtime; repeating full discovery can launch another relay for every read.
use std::{collections::VecDeque, time::Duration};

use orchester_core::connection::{
  app_server::Update,
  app_server_mcp::{McpInventoryError, UnavailableServer, starting_selected, verify_only_chosen_run},
};
use serde_json::{Value, json};
use tokio::time::Instant;

use super::{AppServerFailure, AppServerSession, Phase, io_message, send};
use crate::supervisor::Supervised;

/// Includes time spent waiting for a status request that never answers.
const READY_TIMEOUT: Duration = Duration::from_secs(60);
/// Status polling is paced only after Codex actually reports a transitional state.
const STATUS_INTERVAL: Duration = Duration::from_millis(250);
pub(super) const STATUS_REQUEST: &str = "mcp-readiness";

pub(super) struct Readiness {
  deadline: Instant,
  next_check: Instant,
  awaiting: bool,
  pending: VecDeque<UnavailableServer>,
}

impl Readiness {
  pub(super) fn new() -> Self {
    let now = Instant::now();
    Self {
      deadline: now + READY_TIMEOUT,
      next_check: now,
      awaiting: true,
      pending: VecDeque::new(),
    }
  }
}

impl AppServerSession {
  pub(super) async fn thread_started(
    &mut self,
    result: &Value,
    process: &Supervised,
  ) -> Result<Vec<Update>, AppServerFailure> {
    self.check_thread(result)?;
    let thread = result
      .pointer("/thread/id")
      .and_then(Value::as_str)
      .ok_or_else(|| "Codex App Server omitted the thread ID".to_owned())?
      .to_owned();
    self.thread = Some(thread.clone());
    self.phase = Phase::Servers;
    if self.setup.tool_approvals {
      self.readiness = Some(Readiness::new());
    }
    send(
      process,
      json!({"id":4,"method":"mcpServerStatus/list","params":{
        "threadId":thread,"limit":1000
      }}),
    )
    .await
    .map_err(|error| io_message(&error))?;
    Ok(vec![Update::Activity(
      orchester_core::connection::stream::AgentEvent::SessionStarted { session: thread },
    )])
  }
  fn readiness_deadline(&mut self) -> Result<(), AppServerFailure> {
    let Some(ready) = &self.readiness else {
      return Ok(());
    };
    if Instant::now() < ready.deadline {
      return Ok(());
    }
    let server = ready.pending.front().cloned().unwrap_or_else(|| UnavailableServer {
      name: self
        .setup
        .expected_servers
        .iter()
        .next()
        .cloned()
        .unwrap_or_else(|| "inventory".to_owned()),
      state: None,
      error: None,
    });
    self.readiness = None;
    self.phase = Phase::Complete;
    Err(
      self
        .project_named(McpInventoryError::ReadinessTimeout(server))
        .to_string()
        .into(),
    )
  }
  pub(super) async fn servers_received(
    &mut self,
    result: &Value,
    targeted: bool,
    process: &Supervised,
  ) -> Result<Vec<Update>, AppServerFailure> {
    self.readiness_deadline()?;
    if self.setup.tool_approvals {
      verify_only_chosen_run(result, &self.setup.expected_servers).map_err(|error| error.to_string())?;
      let selected = if targeted {
        self
          .readiness
          .as_ref()
          .and_then(|ready| ready.pending.front())
          .map(|server| std::collections::BTreeSet::from([server.name.clone()]))
          .ok_or_else(|| "Codex sent an unexpected MCP readiness response".to_owned())?
      } else {
        self.setup.expected_servers.clone()
      };
      let pending = starting_selected(result, &selected, |text| self.one_line(text))
        .map_err(|error| self.project_named(error).to_string())?;
      let ready = self
        .readiness
        .as_mut()
        .expect("required inventory starts a readiness deadline");
      if targeted {
        ready.pending.pop_front();
      }
      ready.pending.extend(pending);
      ready.awaiting = false;
      ready.next_check = Instant::now() + STATUS_INTERVAL;
      if !ready.pending.is_empty() {
        return Ok(Vec::new());
      }
    }
    let left_out = if targeted {
      Vec::new()
    } else {
      self.check_servers(result)?
    };
    self.readiness = None;
    self.phase = Phase::Turn;
    self.start_turn(process).await.map_err(|error| io_message(&error))?;
    Ok(left_out)
  }

  /// Called by the worker's regular beat; never waits for a response, so cancellation stays active.
  pub(in crate::execution) async fn poll_servers(&mut self, process: &Supervised) -> Result<(), AppServerFailure> {
    self.readiness_deadline()?;
    let Some(ready) = &self.readiness else {
      return Ok(());
    };
    if ready.awaiting || Instant::now() < ready.next_check {
      return Ok(());
    }
    let server = ready
      .pending
      .front()
      .expect("only pending servers keep readiness active")
      .name
      .clone();
    send(
      process,
      json!({"id":STATUS_REQUEST,"method":"mcpServerStatus/list","params":{
        "threadId":self.thread,"serverName":server,"limit":1000
      }}),
    )
    .await
    .map_err(|error| io_message(&error))?;
    self.readiness.as_mut().expect("readiness is active").awaiting = true;
    Ok(())
  }
}
