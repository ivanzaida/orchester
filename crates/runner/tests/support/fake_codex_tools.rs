//! How `orchester-fake-codex` uses Orchester's tools ("Use Orchester's tools"), as Codex does: it starts the
//! server its `-c mcp_servers.Orchester.…` overrides configure, handing it the variables `env_vars` names from
//! its own environment, lists the tools (`orchester-tools: <names>`), calls `git_status`, then reads
//! `outside-notes.txt` next to the repository's `.orchester` folder with `read_outside_file`
//! (`orchester-call: <tool> <ok|error>: <first line>`). Codex sends no approval request for a tool whose
//! approval mode is `approve`, so neither does the fake: the runner asks about the read before it answers.

use std::{path::Path, process::Command};

use serde_json::json;

use super::fake_permission_prompt::Prompt;

const KEY: &str = "mcp_servers.Orchester.";

pub fn report(args: &[String], worktree: &Path) {
  let setting = |name: &str| -> Option<serde_json::Value> {
    let value = args
      .iter()
      .find_map(|arg| arg.strip_prefix(KEY)?.strip_prefix(name)?.strip_prefix('='))?;
    // The values the runner writes are TOML basic strings and arrays of them, which read as JSON.
    serde_json::from_str(value).ok()
  };
  let (Some(program), Some(arguments), Some(address), Some(handed)) = (
    setting("command"),
    setting("args"),
    setting("env.ORCHESTER_APPROVAL_ADDRESS"),
    setting("env_vars"),
  ) else {
    eprintln!("orchester-tools: no server");
    return;
  };
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
  let Some(mut server) = Prompt::start(command, "approve") else {
    eprintln!("orchester-tools: did not start");
    return;
  };
  eprintln!("orchester-tools: {}", server.list().unwrap_or_default().join(","));
  let outside = worktree
    .ancestors()
    .nth(3)
    .map(|repository| repository.join("outside-notes.txt").to_string_lossy().into_owned());
  for (tool, arguments) in [
    ("git_status", json!({})),
    ("read_outside_file", json!({ "path": outside })),
  ] {
    match server.call(tool, &arguments) {
      Some((text, failed)) => eprintln!(
        "orchester-call: {tool} {}: {}",
        if failed { "error" } else { "ok" },
        text.lines().next().unwrap_or_default()
      ),
      None => eprintln!("orchester-call: {tool} no answer"),
    }
  }
}
