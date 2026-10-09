//! How `orchester-fake-codex` starts and uses Orchester's server, as Codex 0.159.2 does: it starts the server its
//! `-c mcp_servers.Orchester.…` overrides configure, handing it the variables `env_vars` names from its own
//! environment, and lists its tools ([`start`]) once for the thread and once more for each
//! `mcpServerStatus/list`, which then closes it; a list that has no answer within [`LIST_WAIT`] is Codex's
//! *timed out awaiting tools/list*. With "Use Orchester's tools" the thread's server lists the tools
//! (`orchester-tools: <names>`), then makes the calls outside the worktree of [`fake_tool_calls::external`]
//! (`orchester-call: <tool> <ok|error>: <first line>`). Codex sends no approval request for a tool whose approval
//! mode is `approve`, so neither does the fake: the runner asks about a call that needs it before it answers. With
//! "Use the skill tools" it makes the calls of [`fake_skill_calls`], with "Use the file tools" those of a read-only
//! step ([`fake_tool_calls::files`]).

use std::{path::Path, process::Command, sync::mpsc, thread, time::Duration};

use serde_json::{Value, json};

use super::fake_permission_prompt::Prompt;

#[path = "fake_skill_calls.rs"]
mod fake_skill_calls;
#[path = "fake_tool_calls.rs"]
mod fake_tool_calls;

const KEY: &str = "mcp_servers.Orchester.";
/// How long the fake waits for the tools of Orchester's server; Codex waits 30 seconds. A relay the runner
/// refuses lists no tools only after its connection failed, which takes Windows about two seconds.
const LIST_WAIT: Duration = Duration::from_secs(6);

/// The calls of "Use Orchester's tools", "Use the file tools" (in a read-only step: no edit) and "Use the
/// index tools" (after `src/lib.rs` changed in the worktree, with a file the index does not know).
fn calls(prompt: &str, worktree: &Path) -> Vec<(&'static str, Value)> {
  let named =
    |calls: Vec<(&'static str, &'static str, Value)>| calls.into_iter().map(|(_, tool, input)| (tool, input)).collect();
  if prompt.contains("Use the skill tools") {
    return named(fake_skill_calls::calls(worktree));
  }
  if prompt.contains("Use the index tools") {
    let changed = "// moved down\n// moved down\npub fn parse_config() {}\n";
    if std::fs::write(worktree.join("src").join("lib.rs"), changed).is_err()
      || std::fs::write(worktree.join("src").join("fresh.rs"), "pub fn fresh() {}\n").is_err()
    {
      eprintln!("cannot change the sources");
    }
    return vec![
      ("find_symbol", json!({"name": "parse_config"})),
      ("find_symbol", json!({"name": "config", "kind": "struct"})),
      ("search_file", json!({"term": "parse_config"})),
      ("list_files", json!({"pattern": "*.rs", "path": null})),
    ];
  }
  if prompt.contains("Use the file tools") {
    return named(fake_tool_calls::files(true));
  }
  if prompt.contains("Use the run tools") {
    return named(fake_tool_calls::run_tools());
  }
  if prompt.contains("Run the check twice") {
    return named(fake_tool_calls::check_twice());
  }
  named(fake_tool_calls::external(worktree))
}

/// Orchester's server started by Codex.
pub struct Started {
  /// The server, once its tools were listed.
  pub server: Option<Prompt>,
  /// Its tools, or why Codex has none.
  pub tools: Result<Vec<String>, String>,
}

/// Orchester's server as the arguments configure it, started; `None` when the arguments configure no such
/// server.
pub fn start(args: &[String]) -> Option<Started> {
  let command = command(args)?;
  let (sender, answer) = mpsc::channel();
  // A relay the runner does not serve never answers: the list waits on its own thread.
  thread::spawn(move || {
    let started = Prompt::start(command, "approve").map(|mut server| {
      let tools = server.list();
      (server, tools)
    });
    sender.send(started).ok();
  });
  let (server, tools) = match answer.recv_timeout(LIST_WAIT) {
    Ok(Some((server, Some(tools)))) => (Some(server), Ok(tools)),
    Ok(Some((server, None))) => (Some(server), Err("tools/list failed".to_owned())),
    Ok(None) => (None, Err("MCP startup failed: the server did not start".to_owned())),
    Err(_) => (
      None,
      Err("MCP startup failed: timed out awaiting tools/list after 30s".to_owned()),
    ),
  };
  Some(Started { server, tools })
}

/// The setting `name` of Orchester's server in the arguments.
pub fn setting(args: &[String], name: &str) -> Option<Value> {
  let value = args
    .iter()
    .find_map(|arg| arg.strip_prefix(KEY)?.strip_prefix(name)?.strip_prefix('='))?;
  // The values the runner writes are TOML basic strings and arrays of them, which read as JSON.
  serde_json::from_str(value).ok()
}

/// The command of Orchester's server the arguments configure, with the variables Codex hands it.
fn command(args: &[String]) -> Option<Command> {
  let (program, arguments, address, handed) = (
    setting(args, "command")?,
    setting(args, "args")?,
    setting(args, "env.ORCHESTER_APPROVAL_ADDRESS")?,
    setting(args, "env_vars")?,
  );
  let mut command = Command::new(program.as_str().unwrap_or_default());
  for arg in arguments.as_array().into_iter().flatten() {
    command.arg(arg.as_str().unwrap_or_default());
  }
  command.env("ORCHESTER_APPROVAL_ADDRESS", address.as_str().unwrap_or_default());
  for name in handed.as_array().into_iter().flatten().filter_map(|name| name.as_str()) {
    if let Ok(value) = std::env::var(name) {
      command.env(name, value);
    }
  }
  Some(command)
}

/// Lists and calls Orchester's tools through the thread's server, `None` when the thread has none.
pub fn report(server: Option<&mut Prompt>, worktree: &Path, prompt: &str) {
  let Some(server) = server else {
    eprintln!("orchester-tools: no server");
    return;
  };
  eprintln!("orchester-tools: {}", server.list().unwrap_or_default().join(","));
  for (tool, arguments) in calls(prompt, worktree) {
    match server.call(tool, &arguments) {
      Some((text, failed)) => {
        eprintln!(
          "orchester-call: {tool} {}: {}",
          if failed { "error" } else { "ok" },
          text.lines().next().unwrap_or_default()
        );
        for line in text.lines() {
          eprintln!("orchester-answer| {line}");
        }
      }
      None => eprintln!("orchester-call: {tool} no answer"),
    }
  }
}
