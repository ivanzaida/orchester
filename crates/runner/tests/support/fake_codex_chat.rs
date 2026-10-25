//! `orchester-fake-codex` as a turn of the Orchestrator chat: an App Server whose arguments give it the chat's
//! domain tools as the HTTP server `orchester` (`-c mcp_servers.orchester.url="…"`, its bearer token under the
//! variable `bearer_token_env_var` names). It appends how it was started to `.orchester/fake-chat.log` in its working
//! directory (`codex-argv: `, `system-prompt: ` with the first line of the thread's `baseInstructions`, `thread: `
//! with `new` or the thread it resumed, `mcp-url: `, `mcp-token: present`, and `prompt: ` with the turn's first line),
//! then answers by the message, the prompt's last line:
//!
//! - "Call <tool> <JSON arguments>": calls that tool with the token and replies `Tool said: <result text>`;
//! - "Answer in parts": replies twice, `First part.` and `Second part.`, each as its own message;
//! - "Keep thinking": writes the start of a reply and what its first response spent, then waits until it is ended;
//! - "Fail now": the turn fails;
//! - anything else: replies `Noted: <message>`.
//!
//! Each turn reports usage and completes, as Codex does; App Server then waits for the runner to end it.

#![allow(dead_code, reason = "each fake CLI uses a part of the shared support")]

use std::{fs, io::Write, path::Path};

use serde_json::json;

use super::{Session, Step, complete, fake_codex_work::message, notify};
use crate::fake_chat_mcp::ChatServer;

/// The override that names the chat's server.
const URL: &str = "mcp_servers.orchester.url=";
const BEARER: &str = "mcp_servers.orchester.bearer_token_env_var=";

/// Whether `args` start a turn of the Orchestrator chat.
pub fn is_chat(args: &[String]) -> bool {
  args.iter().any(|arg| arg.starts_with(URL))
}

/// The value of override `key` in `args`, its TOML quotes taken off.
fn setting<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
  args
    .iter()
    .find_map(|arg| arg.strip_prefix(key))
    .map(|value| value.trim_matches('"'))
}

fn server(args: &[String]) -> Option<ChatServer> {
  let url = setting(args, URL)?.to_owned();
  let token = setting(args, BEARER)
    .and_then(std::env::var_os)
    .map(|token| format!("Bearer {}", token.to_string_lossy()));
  Some(ChatServer { url, token })
}

/// Answers the turn whose prompt is `prompt`, in a thread started with `base` as its instructions.
pub fn answer(session: &Session, (args, base): (&[String], Option<&str>), prompt: &str) -> Step {
  let server = server(args);
  log(session, (args, base), server.as_ref(), prompt);
  let said = prompt.lines().last().unwrap_or_default();
  if said.contains("Fail now") {
    complete(session, Some("Something broke on the provider's side."));
    return Step::Go;
  }
  if said.contains("Keep thinking") {
    message(session, "item_wait", "Let me look at the queue.");
    let usage = json!({"inputTokens": 900, "cachedInputTokens": 0, "outputTokens": 40});
    notify(
      session,
      "thread/tokenUsage/updated",
      json!({"tokenUsage": {"last": usage, "total": usage}}),
    );
    super::fake_codex_work::wait_to_be_ended();
  }
  if said.contains("Answer in parts") {
    message(session, "item_first", "First part.");
    message(session, "item_second", "Second part.");
    complete(session, None);
    return Step::Go;
  }
  let text = match said.strip_prefix("Call ") {
    Some(call) => match server.as_ref().map(|server| server.call(call)) {
      Some(Ok(result)) => format!("Tool said: {result}"),
      Some(Err(error)) => format!("The call failed: {error}"),
      None => "No tools were configured.".to_owned(),
    },
    None => format!("Noted: {said}"),
  };
  message(session, "item_chat", &text);
  complete(session, None);
  Step::Go
}

/// Appends how the fake was started to `<cwd>/.orchester/fake-chat.log`.
fn log(session: &Session, (args, base): (&[String], Option<&str>), server: Option<&ChatServer>, prompt: &str) {
  let thread = if session.resumed {
    session.thread.as_str()
  } else {
    "new"
  };
  let mut lines = vec![
    format!("codex-argv: {}", args.join(" ")),
    format!(
      "system-prompt: {}",
      base.and_then(|base| base.lines().next()).unwrap_or_default()
    ),
    format!("thread: {thread}"),
    format!("prompt: {}", prompt.lines().next().unwrap_or_default()),
  ];
  if let Some(server) = server {
    lines.push(format!("mcp-url: {}", server.url));
    if server.token.is_some() {
      lines.push("mcp-token: present".to_owned());
    }
  }
  let folder = Path::new(&session.cwd).join(".orchester");
  let written = fs::create_dir_all(&folder).and_then(|()| {
    let mut file = fs::OpenOptions::new()
      .create(true)
      .append(true)
      .open(folder.join("fake-chat.log"))?;
    writeln!(file, "{}", lines.join("\n"))
  });
  if let Err(error) = written {
    eprintln!("cannot write the chat log: {error}");
  }
}
