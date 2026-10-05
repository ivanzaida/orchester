//! How `orchester-fake-codex` uses Orchester's tools ("Use Orchester's tools"), as Codex does: it starts the
//! server its `-c mcp_servers.Orchester.…` overrides configure, handing it the variables `env_vars` names from
//! its own environment, lists the tools (`orchester-tools: <names>`) and calls `git_status`
//! (`orchester-call: git_status <ok|error>: <first line>`). Codex sends no approval request for a tool whose
//! approval mode is `approve`, so neither does the fake.

use std::process::Command;

use serde_json::json;

use super::fake_permission_prompt::Prompt;

const KEY: &str = "mcp_servers.Orchester.";

pub fn report(args: &[String]) {
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
  match server.call("git_status", &json!({})) {
    Some((text, failed)) => eprintln!(
      "orchester-call: git_status {}: {}",
      if failed { "error" } else { "ok" },
      text.lines().next().unwrap_or_default()
    ),
    None => eprintln!("orchester-call: git_status no answer"),
  }
}
