//! A stand-in for `codex app-server --stdio` in the runner's execution tests, the only way Orchester
//! starts Codex. It speaks the part of the JSON-RPC protocol the runner uses (`initialize`,
//! `account/read`, `config/read`, `thread/start` or `thread/resume`, `mcpServerStatus/list`, `turn/start`) and
//! answers the way Codex 0.159 did in the probes (`.tmp/agent-runtime/`), without a model. What a
//! turn does depends on its prompt (`fake_codex_work`); the requests a person decides are in
//! `fake_app_server`.
//!
//! On standard error it reports what it was given: its arguments (`argv: `), the names of its
//! environment variables, its `PATH`, `CARGO_TARGET_DIR`, its process id and its MCP servers
//! (`fake_mcp`) as it starts; the parameters of `thread/start` or `thread/resume` (`thread: `) and of
//! `turn/start` without the prompt (`turn: `); and the prompt (its first line, then each line). The
//! runner keeps standard error in the activity, so tests can check how it was started.
//!
//! The thread's configuration may approve MCP tools (`mcp_servers.<server>.tools.<tool>.approval_mode =
//! "approve"`); such a tool runs without an approval request, as in Codex 0.159.2.
//!
//! A new session is `fake-thread-1`; `thread/resume` continues the thread it names in the worktree, unless
//! its id has `-gone-` in it: then Codex has no history of it and refuses, as 0.159.2 does.
//!
//! Its configuration (`config/read`) has, besides the project servers of its arguments, the user's own
//! server `user-global`, which runs unless the thread's configuration turns it off. Its layers are the
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
  path::PathBuf,
  process::ExitCode,
  thread,
  time::Duration,
};

use serde_json::{Value, json};

#[path = "fake_app_server.rs"]
mod fake_app_server;
#[path = "fake_codex_work.rs"]
mod fake_codex_work;
#[path = "fake_mcp.rs"]
mod fake_mcp;

/// Set for the helper process, which only waits.
const HELPER: &str = "ORCHESTER_FAKE_CODEX_HELPER";
/// The turn every session works in.
const TURN: &str = "fake-turn-1";

fn main() -> ExitCode {
  if std::env::var_os(HELPER).is_some() {
    thread::sleep(Duration::from_secs(600));
    return ExitCode::SUCCESS;
  }
  let args: Vec<String> = std::env::args().skip(1).collect();
  if args.first().is_none_or(|first| first != "app-server") {
    eprintln!("this fake Codex runs as App Server only");
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
  eprintln!(
    "path: {}",
    std::env::var_os("PATH").unwrap_or_default().to_string_lossy()
  );
  if let Some(target) = std::env::var_os("CARGO_TARGET_DIR") {
    eprintln!("cargo target: {}", PathBuf::from(target).display());
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
  /// The MCP tools the thread's configuration approves (`<server>/<tool>`), which run without a request.
  pub approved_tools: BTreeSet<String>,
  /// The person's decisions the turn waits on, if any.
  pub approvals: Option<fake_app_server::Approvals>,
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
      approved_tools: BTreeSet::new(),
      approvals: None,
    }
  }

  fn receive(&mut self, message: &Value) -> Step {
    let params = &message["params"];
    match message["method"].as_str() {
      Some("initialize") => emit(&json!({"id": 1, "result": {}})),
      Some("initialized") => {}
      Some("account/read") => emit(&json!({"id": 2, "result": {"account": {"type": "chatgpt"}}})),
      Some(method @ ("thread/start" | "thread/resume")) => self.thread(method, params),
      Some("config/read") => {
        let cwd = PathBuf::from(params["cwd"].as_str().unwrap_or_default());
        emit(&json!({"id": 6, "result": {"config": self.configuration(), "layers": self.layers(&cwd)}}));
      }
      Some("mcpServerStatus/list") => emit(&json!({"id": 4, "result": {"data": self.servers(), "nextCursor": null}})),
      Some("turn/start") => return self.turn(params),
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
    self.user_server_off = params["config"]["mcp_servers"]["user-global"]["enabled"] == json!(false);
    self.approved_tools = approved_tools(&params["config"]["mcp_servers"]);
    self.cwd = PathBuf::from(params["cwd"].as_str().unwrap_or_default());
    self.resumed = method == "thread/resume";
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

  /// The effective configuration: the project servers and the user's own `user-global`.
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
    json!({"mcp_servers": servers, "hooks": null})
  }

  /// The configuration's layers: the user's (with `user-global`), the arguments' and, for a worktree
  /// with a `.codex` folder, the project's, disabled while the worktree is untrusted.
  fn layers(&self, cwd: &std::path::Path) -> Vec<Value> {
    let mut layers = vec![
      json!({"name": {"type": "user", "file": "fake-codex-home/config.toml"},
        "config": {"mcp_servers": {"user-global": {"command": "user-tool"}}}}),
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

  /// The project servers, of which `broken` did not start, and the user's own server.
  fn servers(&self) -> Vec<Value> {
    let mut servers: Vec<Value> = self
      .project_servers()
      .into_iter()
      .map(|name| {
        let status = if self.broken && name == "broken" {
          "failed"
        } else {
          "connected"
        };
        json!({"name": name, "runtimeStatus": status, "toolsError": null})
      })
      .collect();
    let user = if self.user_server_off { "disabled" } else { "connected" };
    servers.push(json!({"name": "user-global", "runtimeStatus": user, "toolsError": null}));
    servers
  }

  fn turn(&mut self, params: &Value) -> Step {
    let prompt = params["input"][0]["text"].as_str().unwrap_or_default().to_owned();
    let mut shown = params.clone();
    shown["input"] = Value::Null;
    eprintln!("turn: {shown}");
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
    emit(&json!({"id": 5, "result": {"turn": {"id": TURN}}}));
    emit(&json!({"method": "turn/started", "params": {"threadId": self.thread, "turn": {"id": TURN}}}));
    let structured = !params["outputSchema"].is_null();
    fake_codex_work::work(self, &prompt, structured)
  }
}

/// The tools of `servers` (a thread's `mcp_servers` configuration) whose approval mode is `approve`.
fn approved_tools(servers: &Value) -> BTreeSet<String> {
  let mut approved = BTreeSet::new();
  for (server, settings) in servers.as_object().into_iter().flatten() {
    for (tool, settings) in settings["tools"].as_object().into_iter().flatten() {
      if settings["approval_mode"] == "approve" {
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

/// A notification of the session's turn.
pub fn notify(session: &Session, method: &str, mut params: Value) {
  params["threadId"] = json!(session.thread);
  params["turnId"] = json!(TURN);
  emit(&json!({"method": method, "params": params}));
}

/// A request of the session's turn, with `id`.
pub fn request(session: &Session, id: u64, method: &str, mut params: Value) {
  params["threadId"] = json!(session.thread);
  params["turnId"] = json!(TURN);
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
    None => json!({"id": TURN, "status": "completed"}),
    Some(message) => json!({"id": TURN, "status": "failed", "error": {"message": message}}),
  };
  notify(session, "turn/completed", json!({"turn": turn}));
}
