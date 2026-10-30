//! Bidirectional Codex App Server conversation for one invocation, the only way Codex runs. MCP user
//! input and elicitation requests and command approvals can be answered once; file and standing
//! permission grants remain disabled. A command approval comes from a rule when the step asks before
//! commands, and from the sandbox when a command needs more than it allows (`sandbox_approval`, on
//! whenever the step's other commands ask a person). Unexpected host requests fail the invocation.
//! The MCP servers of the user's own configuration, read with `config/read` before the thread
//! starts, are turned off for the thread, and the thread's inventory must show every server the step
//! does not choose turned off; a configuration with hooks refuses the step
//! (`orchester_core::connection::app_server_mcp`). A chosen server Codex gets by another name, as the user's
//! configuration has one of its name, is named as the project names it in what the step records ([`names`]). The
//! repository's own `.codex` folder must be left
//! out (a project layer Codex loaded refuses the step); one the worktree has is noted in the activity
//! (`orchester_core::connection::app_server_config`). A step that asks before MCP
//! actions needs its project-selected servers connected before the turn begins, any other step works
//! without one that is not and its activity says so. Codex's standard error is kept in the activity
//! as raw lines, bounded and without its echo of the user's settings (`super::stderr`). A person's
//! message sent now joins the running turn with `turn/steer` ([`AppServerSession::steer`]); Codex
//! answers at once and gives it to the model after the tool call that runs (`super::worker::steer`).

use std::{collections::BTreeSet, io, path::Path};

use orchester_core::{
  attachment::input::codex_items,
  command::redact_secrets,
  connection::{
    app_server::{AppServerStream, PendingApproval, Update},
    app_server_config::{refuse_taken_names, refuse_taken_provider, repository_settings},
    app_server_mcp::{refuse_user_hooks, servers_to_turn_off, thread_config},
    stream::{AgentEvent, OutputStream, RunnerProblem},
  },
  resource::RepositoryAccess,
};
use serde_json::{Value, json};
use thiserror::Error;

use super::stderr::StderrLog;

mod checks;
mod names;
mod setup;
pub(in crate::execution) use self::setup::AppServerSetup;
use crate::supervisor::Supervised;

/// Characters kept of Codex's reason for refusing a request.
const REASON_CHARS: usize = 300;
/// How the ids of the runner's `turn/steer` requests start; the rest names the message.
pub(in crate::execution) const STEER_REQUEST: &str = "steer-";
/// How the id of a `turn/steer` request that carries a denial's message starts; the rest names the declined request.
pub(super) const DENIAL_STEER: &str = "steer-denial-";

/// Codex's answer to a `turn/steer` request.
pub(in crate::execution) struct SteerAnswer {
  pub id: String,
  /// Codex's reason when it did not add the message to the turn.
  pub refused: Option<String>,
}

#[derive(Debug, Error)]
pub(in crate::execution) enum AppServerFailure {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
  Initialize,
  Account,
  Configuration,
  Thread,
  Servers,
  Turn,
  Working,
  Complete,
}

pub(in crate::execution) struct AppServerSession {
  setup: AppServerSetup,
  phase: Phase,
  thread: Option<String>,
  turn: Option<String>,
  turn_request: u64,
  turn_replies: BTreeSet<u64>,
  /// MCP completion does not prove that the model collected a yielded approval result.
  approval_results_to_collect: bool,
  /// Turns that ended while Orchester's tools still worked for them: a yielded code-mode cell of one goes on after
  /// its turn and may still ask a decision ([`Self::tools_outlived_turn`]).
  yielded_turns: BTreeSet<String>,
  stream: AppServerStream,
  /// The configured MCP servers the step does not choose, turned off for its thread.
  turned_off: Vec<String>,
  stderr: StderrLog,
  /// Answers to `turn/steer` requests not taken yet.
  steer_answers: Vec<SteerAnswer>,
}

impl AppServerSession {
  pub fn new(setup: AppServerSetup) -> Self {
    Self {
      phase: Phase::Initialize,
      turned_off: Vec::new(),
      stderr: StderrLog::default(),
      steer_answers: Vec::new(),
      thread: None,
      turn: None,
      turn_request: 5,
      turn_replies: BTreeSet::new(),
      approval_results_to_collect: false,
      yielded_turns: BTreeSet::new(),
      stream: AppServerStream::with_exemptions(setup.exemptions.clone()),
      setup,
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

  /// The runner asks a person about a call of Orchester's tool `action` with `arguments`
  /// ([`AppServerStream::runner_asks`]): the id of Codex's call when it has started.
  pub fn runner_asks(&mut self, action: &str, arguments: &serde_json::Value) -> Option<String> {
    self.approval_results_to_collect = true;
    self.stream.runner_asks(action, arguments)
  }

  pub fn complete(&self) -> bool {
    self.phase == Phase::Complete
  }

  /// What the running turn spent so far, as Codex reported it; Codex gives it with the turn's end otherwise.
  pub const fn usage_so_far(&self) -> orchester_core::run::TokenUsage {
    self.stream.usage_so_far()
  }

  /// Whether a turn runs that a message can join.
  pub fn turn_working(&self) -> bool {
    matches!(self.phase, Phase::Turn | Phase::Working) && self.turn.is_some()
  }

  /// The id of the turn Codex started last, which a `turn/steer` expects.
  pub fn turn(&self) -> Option<&str> {
    self.turn.as_deref()
  }

  /// Whether Codex's MCP inventory was checked: Codex has started every server of the thread and of the
  /// inventory.
  pub fn servers_checked(&self) -> bool {
    matches!(self.phase, Phase::Turn | Phase::Working | Phase::Complete)
  }

  /// Whether the turn has not started yet, but will.
  pub fn awaits_turn(&self) -> bool {
    self.phase != Phase::Complete && !self.turn_working()
  }

  /// Adds the input items `input` (a message and its attached files, `codex_items`) to the running turn as request
  /// `id` (`turn/steer`, `expectedTurnId` the running turn); its answer comes with [`Self::take_steer_answers`].
  pub async fn steer(&self, process: &Supervised, id: &str, input: &[Value]) -> io::Result<()> {
    send(
      process,
      json!({"id":id,"method":"turn/steer","params":{
        "threadId":self.thread,"input":input,"expectedTurnId":self.turn
      }}),
    )
    .await
  }

  /// Codex's answers to `turn/steer` requests since the last call.
  pub fn take_steer_answers(&mut self) -> Vec<SteerAnswer> {
    std::mem::take(&mut self.steer_answers)
  }

  /// Calls still running, or approval results that a yielded cell may still hold.
  pub fn has_orchester_results_to_collect(&self) -> bool {
    self.approval_results_to_collect || self.stream.has_active_orchester_tools()
  }

  /// The turn that ended left Orchester's tools working: its cells' requests stay part of the step.
  pub fn tools_outlived_turn(&mut self) {
    self.yielded_turns.extend(self.turn.clone());
  }

  pub fn has_active_tools(&self) -> bool {
    self.stream.has_active_tools()
  }

  pub fn has_other_active_tools(&self, pending: &PendingApproval) -> bool {
    self.stream.has_other_active_tools(pending)
  }

  pub async fn receive(
    &mut self,
    stream: OutputStream,
    line: &str,
    process: &Supervised,
  ) -> Result<Vec<Update>, AppServerFailure> {
    if stream == OutputStream::Stderr {
      let secrets = &self.setup.secrets;
      let kept = self.stderr.keep(line, |text| redact_secrets(text, secrets));
      return Ok(
        kept
          .map(|line| Update::Activity(AgentEvent::Raw { stream, line }))
          .into_iter()
          .collect(),
      );
    }
    let value: Value = serde_json::from_str(line).map_err(|_| "Codex App Server sent invalid JSON".to_owned())?;
    if value.get("method").is_some() {
      if value.get("method").and_then(Value::as_str) == Some("turn/started") {
        self.turn = value
          .pointer("/params/turn/id")
          .and_then(Value::as_str)
          .map(str::to_owned);
      }
      let renamed = self.by_project_names(&value);
      let updates = self
        .stream
        .parse(renamed.as_deref().unwrap_or(line))
        .map_err(|error| error.to_string())?;
      let updates = self.screen(updates)?;
      if updates.iter().any(|update| matches!(update, Update::TurnEnded { .. })) {
        self.phase = Phase::Complete;
      }
      return Ok(updates);
    }
    if let Some(id) = value
      .get("id")
      .and_then(Value::as_str)
      .filter(|id| id.starts_with(STEER_REQUEST))
    {
      let refused = value.get("error").map(|error| self.steer_refusal(error));
      self.steer_answers.push(SteerAnswer {
        id: id.to_owned(),
        refused,
      });
      return Ok(Vec::new());
    }
    self.response(&value, process).await
  }

  /// Fails the step on a decision Codex asks outside the active turn (or a turn whose yielded cells still work), or
  /// about a command this step never leaves to a person. An MCP approval in a step that does not ask before MCP actions
  /// is declined at once, without a person, and the declined call is recorded: Codex 0.159.2 asks it even
  /// with `mcp_elicitations` off.
  fn screen(&self, updates: Vec<Update>) -> Result<Vec<Update>, AppServerFailure> {
    let mut screened = Vec::with_capacity(updates.len());
    for update in updates {
      if let Update::Question(question) = &update
        && self.thread.as_deref() != Some(question.thread.as_str())
      {
        return Err(
          "Codex App Server asked a question outside the active thread"
            .to_owned()
            .into(),
        );
      }
      let Update::Approval(request) = update else {
        screened.push(update);
        continue;
      };
      let running =
        matches!(self.phase, Phase::Turn | Phase::Working) && self.turn.as_deref() == request.turn.as_deref();
      let yielded = request
        .turn
        .as_ref()
        .is_some_and(|turn| self.yielded_turns.contains(turn));
      let active = self.thread.as_deref() == Some(request.thread.as_str()) && (running || yielded);
      if !active || (request.is_command() && !self.setup.command_approvals && !self.setup.sandbox_approvals) {
        return Err(
          "Codex App Server requested a tool decision outside the active turn"
            .to_owned()
            .into(),
        );
      }
      if !request.is_command() && !self.setup.tool_approvals {
        screened.push(Update::Activity(AgentEvent::McpToolDeclined {
          item: request.item().to_owned(),
          server: request.server.clone(),
          tool: request.action.clone(),
        }));
        screened.push(Update::Declined(request.response(false)));
        continue;
      }
      screened.push(Update::Approval(request));
    }
    Ok(screened)
  }

  /// Lines of Codex's standard error dropped once the activity kept as many as it keeps.
  pub const fn stderr_left_out(&self) -> u64 {
    self.stderr.left_out()
  }

  pub fn redact(&self, text: &str) -> String {
    redact_secrets(text, &self.setup.secrets)
  }

  async fn response(&mut self, value: &Value, process: &Supervised) -> Result<Vec<Update>, AppServerFailure> {
    let id = value
      .get("id")
      .and_then(Value::as_u64)
      .ok_or_else(|| "Codex App Server omitted a response ID".to_owned())?;
    if let Some(error) = value.get("error") {
      return Err(self.rejected(error).into());
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
        if self.setup.model_provider.is_none() && result.get("account").is_none_or(Value::is_null) {
          return Err(AppServerFailure::NotSignedIn(
            "Codex is not signed in. Sign in to the normal Codex account and retry this step.".to_owned(),
          ));
        }
        self.phase = Phase::Configuration;
        let cwd = self.setup.worktree.to_string_lossy();
        send(
          process,
          json!({"id":6,"method":"config/read","params":{"cwd":cwd,"includeLayers":true}}),
        )
        .await
        .map_err(|error| io_message(&error))?;
        Ok(Vec::new())
      }
      (Phase::Configuration, 6) => self.configured(result, process).await,
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
        let left_out = self.check_servers(result)?;
        self.phase = Phase::Turn;
        self.start_turn(process).await.map_err(|error| io_message(&error))?;
        Ok(left_out)
      }
      (Phase::Turn | Phase::Working | Phase::Complete, id) if self.turn_replies.remove(&id) => {
        // A previous turn/start reply may arrive after its turn completed and the next one started.
        if id == self.turn_request && self.phase == Phase::Turn {
          self.phase = Phase::Working;
        }
        Ok(Vec::new())
      }
      _ => Err("Codex App Server responded out of sequence".to_owned().into()),
    }
  }

  /// Why the step stops after Codex answered its request with `error`, with Codex's reason
  /// ([`Self::reason`]) and error code; a session Codex cannot resume (its history is not in this Codex
  /// home, for one) is named. Also logged, as the failure is the only trace of what Codex refused.
  fn rejected(&self, error: &Value) -> String {
    let reason = self.reason(error);
    let failure = match (&self.setup.resume, self.phase) {
      (Some(session), Phase::Thread) => format!("Codex could not resume the earlier session {session}: {reason}"),
      _ => {
        let code = error
          .get("code")
          .and_then(Value::as_i64)
          .map_or_else(String::new, |code| format!(" (code {code})"));
        format!("Codex App Server rejected {}{code}: {reason}", phase_name(self.phase))
      }
    };
    tracing::warn!(%failure, "Codex App Server refused a request");
    failure
  }

  /// Codex's message in `error`, its secrets redacted, on one line, cut to [`REASON_CHARS`].
  fn reason(&self, error: &Value) -> String {
    let message = error
      .get("message")
      .and_then(Value::as_str)
      .unwrap_or("no reason given");
    self.one_line(message)
  }

  /// Why Codex refused a `turn/steer`: its reason ([`Self::reason`]), with the error code and the details it gives
  /// (`data`, such as the kind of a turn that cannot be steered), so the log tells the turn's state.
  fn steer_refusal(&self, error: &Value) -> String {
    let mut refusal = self.reason(error);
    if let Some(code) = error.get("code").and_then(Value::as_i64) {
      refusal.push_str(&format!(" (code {code})"));
    }
    if let Some(data) = error.get("data").filter(|data| !data.is_null()) {
      refusal.push_str(&format!(" {}", self.one_line(&data.to_string())));
    }
    refusal
  }

  /// `text`, its secrets redacted, on one line, cut to [`REASON_CHARS`].
  fn one_line(&self, text: &str) -> String {
    let redacted = self.redact(text);
    let line = redacted.split_whitespace().collect::<Vec<_>>().join(" ");
    line.chars().take(REASON_CHARS).collect()
  }

  /// Checks the configuration Codex read (`config/read` with its layers), then starts the thread
  /// with the servers the step does not choose turned off; notes a repository `.codex` folder Codex
  /// left out.
  async fn configured(&mut self, result: &Value, process: &Supervised) -> Result<Vec<Update>, AppServerFailure> {
    refuse_user_hooks(result).map_err(|error| error.to_string())?;
    let ignored = repository_settings(result).map_err(|error| error.to_string())?;
    refuse_taken_names(result, &self.setup.expected_servers).map_err(|error| error.to_string())?;
    if let Some(provider) = &self.setup.model_provider {
      refuse_taken_provider(result, provider).map_err(|error| error.to_string())?;
    }
    self.turned_off = servers_to_turn_off(result, &self.setup.expected_servers).map_err(|error| error.to_string())?;
    self.phase = Phase::Thread;
    self.start_thread(process).await.map_err(|error| io_message(&error))?;
    Ok(
      ignored
        .map(|folder| {
          Update::Activity(AgentEvent::RunnerProblem {
            problem: RunnerProblem::CodexSettingsIgnored {
              folder: in_worktree(&folder, &self.setup.worktree),
            },
          })
        })
        .into_iter()
        .collect(),
    )
  }

  async fn start_thread(&self, process: &Supervised) -> io::Result<()> {
    let sandbox = match self.setup.access {
      RepositoryAccess::ReadOnly => "read-only",
      RepositoryAccess::Write => "workspace-write",
    };
    let policy = json!({"granular":{
      "mcp_elicitations":self.setup.tool_approvals,
      "rules":self.setup.command_approvals,"sandbox_approval":self.setup.sandbox_approvals,
      "request_permissions":false,"skill_approval":false
    }});
    let cwd = self.setup.worktree.to_string_lossy();
    let config = Value::Object(thread_config(&self.turned_off));
    let mut params = match &self.setup.resume {
      Some(thread) => json!({"threadId":thread,"cwd":cwd,"model":self.setup.model,
        "approvalPolicy":policy,"approvalsReviewer":"user","sandbox":sandbox,"config":config}),
      None => json!({"cwd":cwd,"model":self.setup.model,
        "approvalPolicy":policy,"approvalsReviewer":"user","sandbox":sandbox,"config":config}),
    };
    if let Some(provider) = &self.setup.model_provider {
      params["modelProvider"] = json!(provider);
    }
    if let Some(instructions) = &self.setup.base_instructions {
      params["baseInstructions"] = json!(instructions);
    }
    let method = if self.setup.resume.is_some() {
      "thread/resume"
    } else {
      "thread/start"
    };
    send(process, json!({"id":3,"method":method,"params":params})).await
  }

  /// The turn of the prompt, with the files attached to the messages it delivers; a copy that cannot be read fails
  /// the turn's start as a write to Codex would.
  async fn start_turn(&mut self, process: &Supervised) -> io::Result<()> {
    let input = codex_items(&self.setup.prompt, &self.setup.attached)?;
    self.turn_with(process, input).await
  }

  /// A yielded code-mode cell belongs to the thread, not to the model turn that started it.
  pub async fn continue_tools(&mut self, process: &Supervised) -> io::Result<()> {
    self.phase = Phase::Turn;
    self.turn = None;
    self.approval_results_to_collect = false;
    // Request 6 belongs to config/read; each continuation gets a fresh JSON-RPC id.
    self.turn_request = self.turn_request.max(6) + 1;
    let input = codex_items(
      "Collect the results of the Orchester tool calls you already started. If a code-mode exec cell yielded, use functions.wait with its existing cell_id until its result is collected. Continue this step using the approval or denial and tool results. Do not repeat the calls or report the step finished while results are pending.",
      &[],
    )?;
    self.turn_with(process, input).await
  }

  async fn turn_with(&mut self, process: &Supervised, input: Vec<Value>) -> io::Result<()> {
    let mut params = json!({"threadId":self.thread,"input":input,
    "model":self.setup.model,"environments":[{
      "environmentId":"local","cwd":self.setup.worktree,
        "runtimeWorkspaceRoots":self.setup.runtime_roots()
    }]});
    if let Some(effort) = &self.setup.effort {
      params["effort"] = json!(effort);
    }
    if let Some(schema) = &self.setup.output_schema {
      params["outputSchema"] = schema.clone();
    }
    self.turn_replies.insert(self.turn_request);
    send(
      process,
      json!({"id":self.turn_request,"method":"turn/start","params":params}),
    )
    .await
  }
}

/// `folder` as the worktree names it (`.codex\`), or as Codex wrote it when it lies elsewhere.
fn in_worktree(folder: &str, worktree: &Path) -> String {
  Path::new(folder).strip_prefix(worktree).map_or_else(
    |_| folder.to_owned(),
    |relative| format!("{}{}", relative.display(), std::path::MAIN_SEPARATOR),
  )
}

fn phase_name(phase: Phase) -> &'static str {
  match phase {
    Phase::Initialize => "initialize",
    Phase::Account => "account/read",
    Phase::Configuration => "config/read",
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
