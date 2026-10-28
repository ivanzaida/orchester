//! A stand-in for `codex app-server --stdio` in the runner's execution tests, the way Orchester starts
//! Codex agents; started with `exec`, it is auto mode's reviewer instead (`fake_codex_review`). It speaks the part of
//! the JSON-RPC protocol the runner uses (`initialize`, `account/read`, `config/read`, `thread/start` or
//! `thread/resume`, `mcpServerStatus/list`, `turn/start`) and answers the way Codex 0.159 did in the probes
//! (`.tmp/agent-runtime/`), without a model. What a turn does depends on its prompt (`fake_codex_work`), or, started
//! with the Orchestrator chat's server, on the person's message (`fake_codex_chat`); the requests a person decides are
//! in `fake_app_server`.
//!
//! On standard error it reports what it was given: its arguments (`argv: `), the names of its
//! environment variables, its `PATH`, `BUILD_DIR`, its process id and its MCP servers
//! (`fake_mcp`) as it starts; the parameters of `thread/start` or `thread/resume` (`thread: `) and of
//! `turn/start` without the prompt (`turn: `); and the prompt (its first line, then each line). The
//! runner keeps standard error in the activity, so tests can check how it was started.
//!
//! Its arguments may approve MCP tools (`-c mcp_servers.<server>.tools={"<tool>"={approval_mode="approve"},…}`);
//! such a tool runs without an approval request, as in Codex 0.159.2. Like 0.159.2, a thread whose
//! configuration has an `mcp_servers` table loses the arguments' servers, Orchester's among them: the table
//! replaces theirs and merges with the user's only, so a table for a server without a transport (`command` or
//! `url`) that is not the user's own refuses the thread. A dotted key (`mcp_servers.<server>.enabled`)
//! changes that one server only. Orchester's server is started for `mcpServerStatus/list`, which lists its
//! tools, or why it has none, and closes it, and for the thread (`fake_codex_tools`). Codex 0.159.2 starts
//! both at about the same time, in either order; the fake starts the thread's second, so a runner that
//! served only the first relay would leave the thread without Orchester's tools. A model may ask it to test
//! the runner's end of the relay further (`fake_codex_relay`).
//!
//! A model with `-refused-` in its name is refused with a two-line reason that echoes the token of
//! Orchester's server (`ORCHESTER_APPROVAL_TOKEN`), so tests can check how the runner reports a refusal.
//!
//! A new session is `fake-thread-1`, its n-th turn `fake-turn-<n>`; `thread/resume` continues the thread it names in
//! the worktree, unless its id has `-gone-` in it: then Codex has no history of it and refuses, as 0.159.2 does.
//!
//! Its configuration (`config/read`) has, besides the project servers of its arguments, the user's own
//! server `user-global`, which runs unless the thread's configuration turns it off, and the user's own hooks
//! (`hooks.state` entries), turned off only while the arguments have `features.hooks=false`. Its layers are the
//! user's, the arguments', and, when the worktree has a `.codex` folder, a project layer that is
//! disabled only while the arguments mark the worktree untrusted (`projects={"<worktree>"=…}`), as Codex
//! names it.

#![allow(
  clippy::print_stdout,
  clippy::print_stderr,
  reason = "a stand-in for a CLI: printing is what it is for"
)]

use std::{
  collections::BTreeSet,
  io::{self, BufRead, Write},
  path::{Path, PathBuf},
  process::ExitCode,
  thread,
  time::Duration,
};

use serde_json::{Value, json};

#[path = "fake_app_server.rs"]
mod fake_app_server;
#[path = "fake_chat_mcp.rs"]
mod fake_chat_mcp;
#[path = "fake_codex_cell.rs"]
mod fake_codex_cell;
#[path = "fake_codex_chat.rs"]
mod fake_codex_chat;
#[path = "fake_codex_check.rs"]
mod fake_codex_check;
#[path = "fake_codex_index.rs"]
mod fake_codex_index;
#[path = "fake_codex_question.rs"]
mod fake_codex_question;
#[path = "fake_codex_relay.rs"]
mod fake_codex_relay;
#[path = "fake_codex_review.rs"]
mod fake_codex_review;
#[path = "fake_codex_tools.rs"]
mod fake_codex_tools;
#[path = "fake_codex_work.rs"]
mod fake_codex_work;
#[path = "fake_mcp.rs"]
mod fake_mcp;
#[path = "fake_permission_prompt.rs"]
mod fake_permission_prompt;
#[path = "fake_provider.rs"]
mod fake_provider;

/// Set for the helper process, which only waits.
const HELPER: &str = "ORCHESTER_FAKE_CODEX_HELPER";

fn main() -> ExitCode {
  if std::env::var_os(HELPER).is_some() {
    thread::sleep(Duration::from_secs(600));
    return ExitCode::SUCCESS;
  }
  let args: Vec<String> = std::env::args().skip(1).collect();
  if let Some(code) = fake_codex_check::answer(&args) {
    return code;
  }
  if fake_codex_review::is_review(&args) {
    return fake_codex_review::review(&args);
  }
  if args.first().is_none_or(|first| first != "app-server") {
    eprintln!("this fake Codex runs as App Server or as a reviewer only");
    return ExitCode::from(2);
  }
  report_start(&args);
  let broken = fake_mcp::report_codex(&args);
  let mut session = Session::new(args, broken);
  for line in io::stdin().lock().lines() {
    let Ok(line) = line else {
      return ExitCode::from(2);
    };
    let Ok(message) = serde_json::from_str::<Value>(&line) else {
      eprintln!("not JSON: {line}");
      return ExitCode::from(2);
    };
    match session.receive(&message) {
      Step::Go => {}
      Step::Exit(code) => return code,
    }
  }
  ExitCode::SUCCESS
}

/// Reports on standard error how the fake was started.
fn report_start(args: &[String]) {
  let mut names: Vec<String> = std::env::vars_os()
    .map(|(name, _)| name.to_string_lossy().into_owned())
    .collect();
  names.sort();
  eprintln!("argv: {}", args.join(" "));
  eprintln!("env: {}", names.join(","));
  if let Some(folder) = std::env::var_os("CODEX_HOME") {
    eprintln!("account folder: {}", folder.to_string_lossy());
  }
  let table = args.iter().find(|arg| arg.starts_with("model_providers."));
  fake_provider::report(table.cloned(), "ORCHESTER_PROVIDER_API_KEY");
  eprintln!(
    "path: {}",
    std::env::var_os("PATH").unwrap_or_default().to_string_lossy()
  );
  if let Some(folder) = std::env::var_os("BUILD_DIR") {
    eprintln!("build dir: {}", folder.to_string_lossy());
  }
  eprintln!("pid: {}", std::process::id());
}

/// What the fake does after a message.
pub enum Step {
  Go,
  Exit(ExitCode),
}

/// One App Server conversation.
pub struct Session {
  args: Vec<String>,
  /// `fake_mcp` reported a server named `broken`, which does not connect.
  broken: bool,
  pub cwd: PathBuf,
  pub thread: String,
  pub resumed: bool,
  roots: BTreeSet<String>,
  /// The user's server `user-global` was turned off for the thread.
  user_server_off: bool,
  /// The thread keeps the servers of the arguments: its configuration has no `mcp_servers` table.
  argument_servers: bool,
  /// The thread's start of Orchester's server, once its tools were listed (after `mcpServerStatus/list`).
  pub orchester: Option<fake_permission_prompt::Prompt>,
  /// The running or last turn: `fake-turn-<n>` for the session's n-th turn, as each turn of Codex has an id of its
  /// own.
  pub turn: String,
  turns: u32,
  pub cell: Option<fake_codex_cell::Cell>,
  /// A command the yielded cell asked about (`fake_codex_cell`).
  pub cell_command: fake_codex_cell::Command,
  /// Every turn calls one of Orchester's tools (`fake_codex_cell`).
  pub calls_every_turn: bool,
  /// The model already collected synchronous tool results; a continuation reuses its final report.
  pub completed_result: Option<String>,
  deferred_turn_reply: Option<Value>,
  /// What the thread's model asks of the runner's end of the relay.
  probe: fake_codex_relay::Probe,
  /// A connection to it that never shows the token, kept open.
  silent: Option<std::net::TcpStream>,
  /// The MCP tools the thread's configuration approves (`<server>/<tool>`), which run without a request.
  pub approved_tools: BTreeSet<String>,
  /// The person's decisions the turn waits on, if any.
  pub approvals: Option<fake_app_server::Approvals>,
  /// The turn waits for a message sent now (`turn/steer`): its prompt, whether it has an output
  /// schema, and whether the steer is refused.
  pub awaits_message: Option<fake_codex_work::AwaitsMessage>,
  /// A turn of the Orchestrator chat waits for a message sent now: whether it refuses it (`fake_codex_chat`).
  pub chat_awaits: Option<bool>,
  /// The thread's `baseInstructions`, which replace Codex's own.
  base_instructions: Option<String>,
}

impl Session {
  fn new(args: Vec<String>, broken: bool) -> Self {
    Self {
      args,
      broken,
      cwd: PathBuf::new(),
      thread: String::new(),
      resumed: false,
      roots: BTreeSet::new(),
      user_server_off: false,
      argument_servers: false,
      orchester: None,
      turn: String::new(),
      turns: 0,
      cell: None,
      cell_command: fake_codex_cell::Command::default(),
      calls_every_turn: false,
      completed_result: None,
      deferred_turn_reply: None,
      probe: fake_codex_relay::Probe::None,
      silent: None,
      approved_tools: BTreeSet::new(),
      approvals: None,
      awaits_message: None,
      chat_awaits: None,
      base_instructions: None,
    }
  }

  fn receive(&mut self, message: &Value) -> Step {
    let params = &message["params"];
    match message["method"].as_str() {
      Some("initialize") => emit(&json!({"id": 1, "result": {}})),
      Some("initialized") => {}
      // On a model provider of its own Codex needs no sign-in, as with an API key and no login.
      Some("account/read") if fake_provider::codex_provider(&self.args) != "openai" => {
        emit(&json!({"id": 2, "result": {"account": null, "requiresOpenaiAuth": false}}));
      }
      Some("account/read") => emit(&json!({"id": 2, "result": {"account": {"type": "chatgpt"}}})),
      Some(method @ ("thread/start" | "thread/resume")) => self.thread(method, params),
      Some("config/read") => {
        let cwd = PathBuf::from(params["cwd"].as_str().unwrap_or_default());
        emit(&json!({"id": 6, "result": {"config": self.configuration(), "layers": self.layers(&cwd)}}));
      }
      Some("mcpServerStatus/list") => {
        emit(&json!({"id": 4, "result": {"data": self.servers(), "nextCursor": null}}));
        if self.argument_servers && self.probe == fake_codex_relay::Probe::LateRelay {
          // The runner has taken the list's answer before the thread's relay comes.
          thread::sleep(Duration::from_millis(500));
          self.start_orchester();
        }
      }
      Some("turn/start") => return self.turn(&message["id"], params),
      Some("turn/steer") if fake_codex_chat::is_chat(&self.args) => {
        return fake_codex_chat::steered(self, &message["id"], params);
      }
      Some("turn/steer") => return fake_codex_work::steered(self, &message["id"], params),
      None if message["id"] == fake_codex_cell::COMMAND => return fake_codex_cell::command_answered(self, message),
      None if fake_codex_chat::is_chat(&self.args) && fake_codex_chat::is_answer(message) => {
        return fake_codex_chat::answered(self, message);
      }
      None => return fake_app_server::answered(self, message),
      Some(other) => {
        eprintln!("unexpected request: {other}");
        return Step::Exit(ExitCode::from(2));
      }
    }
    Step::Go
  }

  /// Answers `thread/start` or `thread/resume` with what Codex activated: the sandbox and policy
  /// asked for, the network and writable roots of the arguments, the local environment.
  fn thread(&mut self, method: &str, params: &Value) {
    eprintln!("thread: {params}");
    if let Some(gone) = params["threadId"].as_str().filter(|id| id.contains("-gone-")) {
      emit(&json!({"id": 3, "error": {"code": -32600, "message": format!("no rollout found for thread id {gone}")}}));
      return;
    }
    if let Some(model) = params["model"].as_str().filter(|model| model.contains("-refused-")) {
      let token = std::env::var("ORCHESTER_APPROVAL_TOKEN").unwrap_or_default();
      let message = format!("model {model} is refused\nfor the token {token}\n");
      emit(&json!({"id": 3, "error": {"code": -32600, "message": message}}));
      return;
    }
    let config = &params["config"];
    if let Some(server) = without_transport(&config["mcp_servers"]) {
      let message = format!("failed to load configuration: invalid transport\nin `mcp_servers.{server}`\n");
      emit(&json!({"id": 3, "error": {"code": -32600, "message": message}}));
      return;
    }
    self.user_server_off = config["mcp_servers"]["user-global"]["enabled"] == json!(false)
      || config["mcp_servers.user-global.enabled"] == json!(false);
    self.argument_servers = config.get("mcp_servers").is_none();
    self.probe = fake_codex_relay::Probe::of(params["model"].as_str().unwrap_or_default());
    self.approved_tools = approved_tools(&self.args);
    self.cwd = PathBuf::from(params["cwd"].as_str().unwrap_or_default());
    self.resumed = method == "thread/resume";
    self.base_instructions = params["baseInstructions"].as_str().map(str::to_owned);
    self.thread = params["threadId"].as_str().unwrap_or("fake-thread-1").to_owned();
    let network = self
      .args
      .iter()
      .any(|arg| arg == "sandbox_workspace_write.network_access=true");
    let configured: BTreeSet<String> = self
      .args
      .iter()
      .find_map(|arg| arg.strip_prefix("sandbox_workspace_write.writable_roots="))
      .and_then(|roots| serde_json::from_str(roots).ok())
      .unwrap_or_default();
    // Native Codex normalizes Windows separators while the runner's worktree contains both.
    let normalize = |path: &str| {
      if cfg!(windows) {
        path.replace('/', "\\")
      } else {
        path.to_owned()
      }
    };
    let cwd = normalize(&self.cwd.to_string_lossy());
    let mut roots: BTreeSet<String> = configured.iter().map(|path| normalize(path)).collect();
    let extra: Vec<String> = roots.iter().filter(|root| **root != cwd).cloned().collect();
    roots.insert(cwd.clone());
    let sandbox = match params["sandbox"].as_str() {
      Some("read-only") => json!({"type": "readOnly", "networkAccess": false}),
      _ => json!({"type": "workspaceWrite", "networkAccess": network, "writableRoots": extra}),
    };
    emit(&json!({"id": 3, "result": {
      "thread": {"id": self.thread, "environments": [{"environmentId": "local", "cwd": cwd, "runtimeWorkspaceRoots": roots}]},
      "approvalPolicy": params["approvalPolicy"], "approvalsReviewer": "user", "model": params["model"],
      "modelProvider": fake_provider::reported(&self.args, params["modelProvider"].as_str()),
      "sandbox": sandbox
    }}));
    self.roots = configured;
    self.roots.insert(self.cwd.to_string_lossy().into_owned());
  }

  /// The project servers of the arguments.
  fn project_servers(&self) -> BTreeSet<&str> {
    self
      .args
      .iter()
      .filter_map(|arg| arg.strip_prefix("mcp_servers.")?.split('.').next())
      .collect()
  }

  /// The effective configuration: the project servers, the user's own `user-global`, and the user's own hooks as
  /// Codex 0.159.2 reported them for a person whose other agent tool installed global ones (2026-10-28): every event
  /// with an empty list and the trust `state` of each, with `features.hooks = false` only while the arguments turn
  /// hooks off.
  fn configuration(&self) -> Value {
    let mut servers: serde_json::Map<String, Value> = self
      .project_servers()
      .into_iter()
      .map(|name| (name.to_owned(), json!({"enabled": true})))
      .collect();
    servers.insert(
      "user-global".to_owned(),
      json!({"enabled": true, "command": "user-tool"}),
    );
    let hooks = json!({
      "PreToolUse": [], "PermissionRequest": [], "PostToolUse": [], "SessionStart": [], "UserPromptSubmit": [],
      "Stop": [],
      "state": {
        "fake-codex-home/hooks.json:session_start:0:0": {"trusted_hash": "sha256:5e1f"},
        "fake-codex-home/hooks.json:pre_tool_use:0:0": {"trusted_hash": "sha256:9a0c"}
      }
    });
    let mut config = json!({"mcp_servers": servers, "hooks": hooks, "features": {}});
    if self.args.iter().any(|arg| arg == "features.hooks=false") {
      config["features"]["hooks"] = json!(false);
    }
    config
  }

  /// The configuration's layers: the user's (with `user-global`), the arguments' and, for a worktree
  /// with a `.codex` folder, the project's, disabled while the worktree is untrusted.
  fn layers(&self, cwd: &std::path::Path) -> Vec<Value> {
    let mut user = json!({"mcp_servers": {"user-global": {"command": "user-tool"}}});
    if fake_provider::clashes(&self.args) {
      user["model_providers"] = json!({"orchester": {"http_headers": {"X-Mine": "1"}}});
    }
    let mut layers = vec![
      json!({"name": {"type": "user", "file": "fake-codex-home/config.toml"}, "config": user}),
      json!({"name": {"type": "sessionFlags"}, "config": {}}),
    ];
    let folder = cwd.join(".codex");
    if folder.is_dir() {
      let mut key = cwd.to_string_lossy().into_owned();
      if cfg!(windows) {
        key = key.replace('/', "\\");
        if let Some(drive) = key.get(..1) {
          key = format!("{}{}", drive.to_ascii_lowercase(), &key[1..]);
        }
      }
      let quoted = serde_json::to_string(&key).expect("a string");
      let untrusted = self
        .args
        .iter()
        .any(|arg| arg.starts_with("projects={") && arg.contains(&format!("{quoted}={{trust_level=\"untrusted\"}}")));
      // A test makes the fake load the folder whatever its trust, as a Codex that changed would.
      let loads_anyway = folder.join("fake-loads-anyway").exists();
      let reason =
        (untrusted && !loads_anyway).then(|| format!("{key} is marked as untrusted in the effective configuration."));
      layers
        .push(json!({"name": {"type": "project", "dotCodexFolder": folder}, "config": {}, "disabledReason": reason}));
    }
    layers
  }

  /// The project servers the thread kept, of which `broken` did not start and Orchester's is started
  /// to list its tools, then for the thread, and the user's own server.
  fn servers(&mut self) -> Vec<Value> {
    if self.argument_servers {
      self.silent = self.probe.before_relays(&self.args);
    }
    let kept = if self.argument_servers {
      self.project_servers()
    } else {
      BTreeSet::new()
    };
    let mut servers: Vec<Value> = kept
      .into_iter()
      .map(|name| match name {
        "Orchester" => orchester_status(&self.args),
        "broken" if self.broken => json!({"name": name, "runtimeStatus": "failed", "toolsError": null}),
        _ => json!({"name": name, "runtimeStatus": "connected", "toolsError": null}),
      })
      .collect();
    if self.argument_servers && self.probe.starts_thread_relay_with_the_list() {
      self.start_orchester();
      if self.probe == fake_codex_relay::Probe::Knock {
        eprintln!("knock: {}", fake_codex_relay::knock(&self.args, None));
      }
    }
    let user = if self.user_server_off { "disabled" } else { "connected" };
    servers.push(json!({"name": "user-global", "runtimeStatus": user, "toolsError": null}));
    servers
  }

  /// Orchester's server for the thread, unless it was started.
  fn start_orchester(&mut self) {
    if self.orchester.is_none() {
      self.orchester = fake_codex_tools::start(&self.args).and_then(|started| started.server);
    }
  }

  fn turn(&mut self, id: &Value, params: &Value) -> Step {
    if self.probe == fake_codex_relay::Probe::Knock {
      eprintln!("late-knock: {}", fake_codex_relay::knock(&self.args, None));
    }
    let prompt = params["input"][0]["text"].as_str().unwrap_or_default().to_owned();
    let mut shown = params.clone();
    shown["input"] = Value::Null;
    eprintln!("turn: {shown}");
    eprintln!("input-blocks: {}", input_blocks(&params["input"]));
    eprintln!("prompt: {}", prompt.lines().next().unwrap_or_default());
    for line in prompt.lines() {
      eprintln!("prompt| {line}");
    }
    let selected: BTreeSet<PathBuf> = params["environments"][0]["runtimeWorkspaceRoots"]
      .as_array()
      .into_iter()
      .flatten()
      .filter_map(Value::as_str)
      .map(PathBuf::from)
      .collect();
    let expected: BTreeSet<PathBuf> = self.roots.iter().map(PathBuf::from).collect();
    if selected != expected {
      eprintln!("the turn's workspace roots {selected:?} are not the thread's {expected:?}");
      return Step::Exit(ExitCode::from(2));
    }
    if let Some(previous) = self.deferred_turn_reply.take() {
      emit(&json!({"id": previous, "result": {"turn": {"id": self.turn}}}));
    }
    self.turns += 1;
    self.turn = format!("fake-turn-{}", self.turns);
    eprintln!("turn-request: {id}");
    if prompt.contains("Delay turn reply") {
      self.deferred_turn_reply = Some(id.clone());
    } else {
      emit(&json!({"id": id, "result": {"turn": {"id": self.turn}}}));
    }
    emit(&json!({"method": "turn/started", "params": {"threadId": self.thread, "turn": {"id": self.turn}}}));
    if fake_codex_chat::is_chat(&self.args) {
      let (args, base) = (self.args.clone(), self.base_instructions.clone());
      return fake_codex_chat::answer(self, (&args, base.as_deref()), &prompt);
    }
    let structured = !params["outputSchema"].is_null();
    fake_codex_work::work(self, &prompt, structured)
  }
}

/// Orchester's server in `mcpServerStatus/list`: started again, its tools listed, then closed.
fn orchester_status(args: &[String]) -> Value {
  match fake_codex_tools::start(args).map(|started| started.tools) {
    Some(Ok(tools)) => {
      let tools: serde_json::Map<String, Value> = tools
        .into_iter()
        .map(|tool| (tool.clone(), json!({"name": tool})))
        .collect();
      json!({"name": "Orchester", "runtimeStatus": "connected", "toolsError": null, "tools": tools})
    }
    Some(Err(error)) => json!({"name": "Orchester", "runtimeStatus": "connected", "toolsError": error}),
    None => json!({"name": "Orchester", "runtimeStatus": "failed", "toolsError": "no command"}),
  }
}

/// A server of `servers` (a thread's `mcp_servers` configuration) Codex 0.159.2 refuses: one with
/// neither a `command` nor a `url` that the user's configuration does not define.
fn without_transport(servers: &Value) -> Option<&str> {
  servers
    .as_object()
    .into_iter()
    .flatten()
    .find(|(name, table)| *name != "user-global" && table.get("command").is_none() && table.get("url").is_none())
    .map(|(name, _)| name.as_str())
}

/// The tools the arguments approve (`<server>/<tool>`), which run without a request: each
/// `mcp_servers.<server>.tools={"<tool>"={approval_mode="approve"},…}`.
fn approved_tools(args: &[String]) -> BTreeSet<String> {
  const APPROVE: &str = "={approval_mode=\"approve\"}";
  let mut approved = BTreeSet::new();
  for arg in args {
    let Some((server, tools)) = arg
      .strip_prefix("mcp_servers.")
      .and_then(|rest| rest.split_once(".tools={"))
    else {
      continue;
    };
    let tools = tools.strip_suffix('}').unwrap_or(tools);
    for entry in tools.split(APPROVE) {
      let quoted = entry.trim_start_matches(',');
      if let Ok(tool) = serde_json::from_str::<String>(quoted) {
        approved.insert(format!("{server}/{tool}"));
      }
    }
  }
  approved
}

/// Writes one protocol line.
pub fn emit(value: &Value) {
  println!("{value}");
  if io::stdout().flush().is_err() {
    eprintln!("cannot flush standard output");
  }
}

/// The input items of a turn by kind, a `localImage` with whether its file is there: `text,text,localImage:there`.
fn input_blocks(input: &Value) -> String {
  input
    .as_array()
    .into_iter()
    .flatten()
    .map(|item| match (item["type"].as_str(), item["path"].as_str()) {
      (Some("localImage"), Some(path)) => {
        format!(
          "localImage:{}",
          if Path::new(path).is_file() { "there" } else { "missing" }
        )
      }
      (kind, _) => kind.unwrap_or("?").to_owned(),
    })
    .collect::<Vec<_>>()
    .join(",")
}

/// A notification of the session's turn.
pub fn notify(session: &Session, method: &str, mut params: Value) {
  params["threadId"] = json!(session.thread);
  params["turnId"] = json!(session.turn);
  emit(&json!({"method": method, "params": params}));
}

/// A request of the session's turn, with `id`.
pub fn request(session: &Session, id: u64, method: &str, mut params: Value) {
  params["threadId"] = json!(session.thread);
  params["turnId"] = json!(session.turn);
  emit(&json!({"id": id, "method": method, "params": params}));
}

/// Ends the turn: its usage, then `turn/completed` with `status`, failed with `error`.
pub fn complete(session: &Session, error: Option<&str>) {
  let usage = json!({"inputTokens": 12_166, "cachedInputTokens": 2_432, "outputTokens": 5});
  notify(
    session,
    "thread/tokenUsage/updated",
    // One model response of 9,739 counted tokens.
    json!({"tokenUsage": {"last": usage, "total": usage}}),
  );
  let turn = match error {
    None => json!({"id": session.turn, "status": "completed"}),
    Some(message) => json!({"id": session.turn, "status": "failed", "error": {"message": message}}),
  };
  notify(session, "turn/completed", json!({"turn": turn}));
}
