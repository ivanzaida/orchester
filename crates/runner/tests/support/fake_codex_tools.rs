//! How `orchester-fake-codex` starts and uses Orchester's server, as Codex 0.159.2 does: it starts the server its
//! `-c mcp_servers.Orchester.…` overrides configure, handing it the variables `env_vars` names from its own
//! environment, and lists its tools ([`start`]) once for the thread and once more for each
//! `mcpServerStatus/list`, which then closes it; a list that has no answer within [`LIST_WAIT`] is Codex's
//! *timed out awaiting tools/list*. With "Use Orchester's tools" the thread's server lists the tools
//! (`orchester-tools: <names>`), calls `git_status`, then reads `outside-notes.txt` next to the repository's
//! `.orchester` folder with `read_outside_file` (`orchester-call: <tool> <ok|error>: <first line>`). Codex sends
//! no approval request for a tool whose approval mode is `approve`, so neither does the fake: the runner asks
//! about the read before it answers.

use std::{path::Path, process::Command, sync::mpsc, thread, time::Duration};

use serde_json::{Value, json};

use super::fake_permission_prompt::Prompt;

const KEY: &str = "mcp_servers.Orchester.";
/// How long the fake waits for the tools of Orchester's server; Codex waits 30 seconds.
const LIST_WAIT: Duration = Duration::from_secs(3);

/// The calls of "Use Orchester's tools", "Use the file tools" (in a read-only step: no edit) and "Use the
/// index tools" (after `src/lib.rs` changed in the worktree, with a file the index does not know).
fn calls(prompt: &str, worktree: &Path) -> Vec<(&'static str, Value)> {
  if prompt.contains("Use the index tools") {
    let changed = "// moved down
// moved down
pub fn parse_config() {}
";
    if std::fs::write(worktree.join("src").join("lib.rs"), changed).is_err()
      || std::fs::write(
        worktree.join("src").join("fresh.rs"),
        "pub fn fresh() {}
",
      )
      .is_err()
    {
      eprintln!("cannot change the sources");
    }
    let search =
      json!({"pattern": "parse_config", "path": null, "glob": null, "case_insensitive": null, "context": null});
    return vec![
      ("find_symbol", json!({"name": "parse_config"})),
      ("find_symbol", json!({"name": "config", "kind": "struct"})),
      ("search", search),
      ("list_files", json!({"pattern": "*.rs", "path": null})),
    ];
  }
  if prompt.contains("Use the file tools") {
    return vec![
      ("read_file", json!({"path": "README.md", "offset": null, "limit": null})),
      (
        "edit_file",
        json!({"path": "README.md", "edits": [{"old": "hi", "new": "ho", "replace_all": null}]}),
      ),
      ("write_file", json!({"path": "new.md", "content": "x"})),
    ];
  }
  let outside = worktree
    .ancestors()
    .nth(3)
    .map(|repository| repository.join("outside-notes.txt").to_string_lossy().into_owned());
  vec![
    ("git_status", json!({})),
    ("read_outside_file", json!({ "path": outside })),
  ]
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

/// The command of Orchester's server the arguments configure, with the variables Codex hands it.
fn command(args: &[String]) -> Option<Command> {
  let setting = |name: &str| -> Option<serde_json::Value> {
    let value = args
      .iter()
      .find_map(|arg| arg.strip_prefix(KEY)?.strip_prefix(name)?.strip_prefix('='))?;
    // The values the runner writes are TOML basic strings and arrays of them, which read as JSON.
    serde_json::from_str(value).ok()
  };
  let (program, arguments, address, handed) = (
    setting("command")?,
    setting("args")?,
    setting("env.ORCHESTER_APPROVAL_ADDRESS")?,
    setting("env_vars")?,
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
