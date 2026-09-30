//! Bidirectional Codex App Server conversation for one invocation. MCP user input and elicitation
//! requests and command approvals can be answered once; file and standing permission grants remain disabled. Unexpected
//! host requests fail the invocation. Global MCP servers remain available; project-selected
//! servers must be connected before the turn begins.

use std::{collections::BTreeSet, io, path::PathBuf};

use orchester_core::{
  command::{SecretVariable, redact_secrets},
  connection::{
    app_server::{AppServerStream, Update},
    app_server_mcp::verify_selected_servers,
    stream::{AgentEvent, OutputStream},
  },
  permission::NetworkAccess,
  resource::RepositoryAccess,
};
use serde_json::{Value, json};
use thiserror::Error;

use crate::supervisor::Supervised;

#[derive(Debug, Error)]
pub(super) enum AppServerFailure {
  #[error("{0}")]
  Protocol(String),
  #[error("{0}")]
  NotSignedIn(String),
}

impl From<String> for AppServerFailure {
  fn from(message: String) -> Self {
    Self::Protocol(message)
  }
}

pub(super) struct AppServerSetup {
  pub prompt: String,
  pub model: String,
  pub effort: Option<String>,
  pub worktree: PathBuf,
  pub access: RepositoryAccess,
  pub command_approvals: bool,
  pub tool_approvals: bool,
  pub network: NetworkAccess,
  pub writable_roots: Vec<PathBuf>,
  pub resume: Option<String>,
  pub expected_servers: BTreeSet<String>,
  pub output_schema: Option<Value>,
  pub secrets: Vec<SecretVariable>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
  Initialize,
  Account,
  Thread,
  Servers,
  Turn,
  Working,
  Complete,
}

pub(super) struct AppServerSession {
  setup: AppServerSetup,
  phase: Phase,
  thread: Option<String>,
  turn: Option<String>,
  stream: AppServerStream,
}

impl AppServerSession {
  pub fn new(setup: AppServerSetup) -> Self {
    Self {
      setup,
      phase: Phase::Initialize,
      thread: None,
      turn: None,
      stream: AppServerStream::default(),
    }
  }

  pub async fn start(&self, process: &Supervised) -> io::Result<()> {
    send(
      process,
      json!({"id":1,"method":"initialize","params":{
        "clientInfo":{"name":"orchester","version":"0.1"},
        "capabilities":{"experimentalApi":true}
      }}),
    )
    .await
  }

  pub fn complete(&self) -> bool {
    self.phase == Phase::Complete
  }

  pub async fn receive(
    &mut self,
    stream: OutputStream,
    line: &str,
    process: &Supervised,
  ) -> Result<Vec<Update>, AppServerFailure> {
    if stream == OutputStream::Stderr {
      return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(line).map_err(|_| "Codex App Server sent invalid JSON".to_owned())?;
    if value.get("method").is_some() {
      if value.get("method").and_then(Value::as_str) == Some("turn/started") {
        self.turn = value
          .pointer("/params/turn/id")
          .and_then(Value::as_str)
          .map(str::to_owned);
      }
      let updates = self.stream.parse(line).map_err(|error| error.to_string())?;
      for update in &updates {
        if let Update::Approval(request) = update
          && (!matches!(self.phase, Phase::Turn | Phase::Working)
            || self.thread.as_deref() != Some(request.thread.as_str())
            || self.turn.as_deref() != request.turn.as_deref()
            || (request.is_command() && !self.setup.command_approvals)
            || (!request.is_command() && !self.setup.tool_approvals))
        {
          return Err(
            "Codex App Server requested a tool decision outside the active turn"
              .to_owned()
              .into(),
          );
        }
      }
      if updates.iter().any(|update| matches!(update, Update::TurnEnded)) {
        self.phase = Phase::Complete;
      }
      return Ok(updates);
    }
    self.response(&value, process).await
  }

  pub fn redact(&self, text: &str) -> String {
    redact_secrets(text, &self.setup.secrets)
  }

  async fn response(&mut self, value: &Value, process: &Supervised) -> Result<Vec<Update>, AppServerFailure> {
    let id = value
      .get("id")
      .and_then(Value::as_u64)
      .ok_or_else(|| "Codex App Server omitted a response ID".to_owned())?;
    if value.get("error").is_some() {
      return Err(format!("Codex App Server rejected {}", phase_name(self.phase)).into());
    }
    let result = value
      .get("result")
      .ok_or_else(|| "Codex App Server omitted a response result".to_owned())?;
    match (self.phase, id) {
      (Phase::Initialize, 1) => {
        self.phase = Phase::Account;
        send(process, json!({"method":"initialized"}))
          .await
          .map_err(|error| io_message(&error))?;
        send(
          process,
          json!({"id":2,"method":"account/read","params":{"refreshToken":false}}),
        )
        .await
        .map_err(|error| io_message(&error))?;
        Ok(Vec::new())
      }
      (Phase::Account, 2) => {
        if result.get("account").is_none_or(Value::is_null) {
          return Err(AppServerFailure::NotSignedIn(
            "Codex is not signed in. Sign in to the normal Codex account and retry this step.".to_owned(),
          ));
        }
        self.phase = Phase::Thread;
        self.start_thread(process).await.map_err(|error| io_message(&error))?;
        Ok(Vec::new())
      }
      (Phase::Thread, 3) => {
        self.check_thread(result)?;
        let thread = result
          .get("thread")
          .and_then(|thread| thread.get("id"))
          .and_then(Value::as_str)
          .ok_or_else(|| "Codex App Server omitted the thread ID".to_owned())?
          .to_owned();
        self.thread = Some(thread.clone());
        self.phase = Phase::Servers;
        send(
          process,
          json!({"id":4,"method":"mcpServerStatus/list","params":{
            "threadId":thread,"limit":1000
          }}),
        )
        .await
        .map_err(|error| io_message(&error))?;
        Ok(vec![Update::Activity(AgentEvent::SessionStarted { session: thread })])
      }
      (Phase::Servers, 4) => {
        self.check_servers(result)?;
        self.phase = Phase::Turn;
        self.start_turn(process).await.map_err(|error| io_message(&error))?;
        Ok(Vec::new())
      }
      (Phase::Turn, 5) => {
        self.phase = Phase::Working;
        Ok(Vec::new())
      }
      (Phase::Complete, 5) => Ok(Vec::new()),
      _ => Err("Codex App Server responded out of sequence".to_owned().into()),
    }
  }

  fn check_thread(&self, result: &Value) -> Result<(), String> {
    if let Some(expected) = &self.setup.resume
      && result.pointer("/thread/id").and_then(Value::as_str) != Some(expected.as_str())
    {
      return Err("Codex resumed a different session".to_owned());
    }
    self.check_environment(result)?;
    let granular = result
      .get("approvalPolicy")
      .and_then(|policy| policy.get("granular"))
      .ok_or_else(|| "Codex did not activate granular tool approval policy".to_owned())?;
    let enabled = |key: &str| granular.get(key).and_then(Value::as_bool).unwrap_or(false);
    if enabled("mcp_elicitations") != self.setup.tool_approvals
      || enabled("rules") != self.setup.command_approvals
      || enabled("sandbox_approval")
      || enabled("request_permissions")
      || enabled("skill_approval")
    {
      return Err("Codex activated a broader approval policy than this step permits".to_owned());
    }
    if result.get("approvalsReviewer").and_then(Value::as_str) != Some("user") {
      return Err("Codex did not route tool approvals to the user".to_owned());
    }
    let sandbox = result
      .get("sandbox")
      .and_then(|sandbox| sandbox.get("type"))
      .and_then(Value::as_str);
    let expected = match self.setup.access {
      RepositoryAccess::ReadOnly => "readOnly",
      RepositoryAccess::Write => "workspaceWrite",
    };
    if sandbox != Some(expected) {
      return Err("Codex activated an unexpected sandbox".to_owned());
    }
    let network = result
      .get("sandbox")
      .and_then(|sandbox| sandbox.get("networkAccess"))
      .and_then(Value::as_bool)
      .unwrap_or(false);
    if network != (self.setup.access == RepositoryAccess::Write && self.setup.network == NetworkAccess::On) {
      return Err("Codex activated unexpected network access".to_owned());
    }
    if self.setup.access == RepositoryAccess::Write {
      let roots = result
        .get("sandbox")
        .and_then(|sandbox| sandbox.get("writableRoots"))
        .and_then(Value::as_array)
        .ok_or_else(|| "Codex omitted writable sandbox roots".to_owned())?;
      let actual: Option<BTreeSet<_>> = roots.iter().map(|root| root.as_str().map(str::to_owned)).collect();
      let mut actual = actual.ok_or_else(|| "Codex reported invalid writable sandbox roots".to_owned())?;
      // Codex reports the local cwd as an environment workspace root, not an extra sandbox root.
      actual.insert(self.setup.worktree.to_string_lossy().into_owned());
      let expected: BTreeSet<String> = self
        .setup
        .writable_roots
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
      let mut allowed = expected;
      allowed.insert(self.setup.worktree.to_string_lossy().into_owned());
      if actual != allowed {
        return Err("Codex writable roots do not match this step's grants".to_owned());
      }
    }
    if result.get("model").and_then(Value::as_str) != Some(&self.setup.model) {
      return Err("Codex selected a different model for the tool-approval step".to_owned());
    }
    Ok(())
  }

  fn check_environment(&self, result: &Value) -> Result<(), String> {
    let environments = result
      .pointer("/thread/environments")
      .and_then(Value::as_array)
      .ok_or_else(|| "Codex did not expose the local environment".to_owned())?;
    let [local] = environments.as_slice() else {
      return Err("Codex did not select exactly one local environment".to_owned());
    };
    let cwd = self.setup.worktree.to_string_lossy();
    if local.get("environmentId").and_then(Value::as_str) != Some("local")
      || local.get("cwd").and_then(Value::as_str) != Some(cwd.as_ref())
      || local.get("runtimeWorkspaceRoots") != Some(&json!([cwd]))
    {
      return Err("Codex selected an environment outside this worktree".to_owned());
    }
    Ok(())
  }

  fn check_servers(&self, result: &Value) -> Result<(), String> {
    verify_selected_servers(result, &self.setup.expected_servers).map_err(|error| error.to_string())
  }

  async fn start_thread(&self, process: &Supervised) -> io::Result<()> {
    let sandbox = match self.setup.access {
      RepositoryAccess::ReadOnly => "read-only",
      RepositoryAccess::Write => "workspace-write",
    };
    let policy = json!({"granular":{
      "mcp_elicitations":self.setup.tool_approvals,
      "rules":self.setup.command_approvals,"sandbox_approval":false,
      "request_permissions":false,"skill_approval":false
    }});
    let cwd = self.setup.worktree.to_string_lossy();
    let params = match &self.setup.resume {
      Some(thread) => json!({"threadId":thread,"cwd":cwd,"model":self.setup.model,
        "approvalPolicy":policy,"approvalsReviewer":"user","sandbox":sandbox}),
      None => json!({"cwd":cwd,"model":self.setup.model,
        "approvalPolicy":policy,"approvalsReviewer":"user","sandbox":sandbox,
        "config":{"features":{"apps":false}}}),
    };
    let method = if self.setup.resume.is_some() {
      "thread/resume"
    } else {
      "thread/start"
    };
    send(process, json!({"id":3,"method":method,"params":params})).await
  }

  async fn start_turn(&self, process: &Supervised) -> io::Result<()> {
    let mut params = json!({"threadId":self.thread,"input":[{"type":"text","text":self.setup.prompt}],
    "model":self.setup.model,"environments":[{
      "environmentId":"local","cwd":self.setup.worktree,
      "runtimeWorkspaceRoots":[self.setup.worktree]
    }]});
    if let Some(effort) = &self.setup.effort {
      params["effort"] = json!(effort);
    }
    if let Some(schema) = &self.setup.output_schema {
      params["outputSchema"] = schema.clone();
    }
    send(process, json!({"id":5,"method":"turn/start","params":params})).await
  }
}

fn phase_name(phase: Phase) -> &'static str {
  match phase {
    Phase::Initialize => "initialize",
    Phase::Account => "account/read",
    Phase::Thread => "thread/start or resume",
    Phase::Servers => "mcpServerStatus/list",
    Phase::Turn => "turn/start",
    Phase::Working | Phase::Complete => "a request",
  }
}

fn io_message(error: &io::Error) -> String {
  format!("cannot write to Codex App Server: {error}")
}

async fn send(process: &Supervised, value: Value) -> io::Result<()> {
  process.send_line(value.to_string()).await
}
