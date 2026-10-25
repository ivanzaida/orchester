//! `orchester-fake-claude` as a turn of the Orchestrator chat: started with `--system-prompt-file` and an
//! `--mcp-config` of the chat's domain tools. It appends how it was started to `.orchester/fake-chat.log` in its
//! working directory (`argv: `, `system-prompt: ` with the file's first line, `mcp-url: `, and `mcp-token: present`
//! when the configuration carries a bearer token; on an API provider `provider: ` with the base URL and the models of
//! its aliases and subagents, `provider key: present` when `ANTHROPIC_AUTH_TOKEN` is set, and `anthropic-env: ` with
//! the names of every `ANTHROPIC_*` variable), then answers by the message:
//!
//! - "Call <tool> <JSON arguments>": calls that tool of the configured server over streamable HTTP with the bearer
//!   token and replies with what it answered (`Tool said: <result text>`);
//! - "Keep thinking": writes the start of a reply and waits until it is ended;
//! - "Fail now": ends with an error result and exit code 1;
//! - "Say the key": replies with the provider's key, and "Fail with the key" prints it on standard error and exits with
//!   1: the runner must keep it out of what the chat keeps;
//! - anything else: replies `Noted: <message>`.
//!
//! Each answer ends with a result that reports usage, as Claude Code does.

#![allow(dead_code, reason = "each fake CLI uses a part of the shared support")]

use std::{fs, io::Write, path::Path, process::ExitCode, thread, time::Duration};

use serde_json::{Value, json};

use crate::fake_chat_mcp::ChatServer;

const USAGE: &str = r#"{"input_tokens":12,"output_tokens":27}"#;
/// The variable Claude Code reads an API provider's key from.
const KEY_VARIABLE: &str = "ANTHROPIC_AUTH_TOKEN";

/// Whether `args` start a turn of the Orchestrator chat.
pub fn is_chat(args: &[String]) -> bool {
  args.iter().any(|arg| arg == "--system-prompt-file") && args.iter().any(|arg| arg == "--mcp-config")
}

/// Answers `message` in session `session`.
pub fn answer(args: &[String], session: &str, message: &str) -> ExitCode {
  let server = server(args);
  log(args, server.as_ref());
  println!(
    r#"{{"type":"system","subtype":"init","cwd":"project","session_id":"{session}","tools":[],"mcp_servers":[{{"name":"orchester","status":"connected"}}],"model":"claude-haiku-4-5","permissionMode":"default","apiKeySource":"none","claude_code_version":"2.1.89"}}"#
  );
  replay(session, message);
  if message.contains("Keep thinking") {
    reply(session, "msg_wait", "Let me look at the queue.");
    loop {
      thread::sleep(Duration::from_secs(1));
    }
  }
  if message.contains("Fail now") {
    result(session, true, "Something broke on the provider's side.");
    return ExitCode::from(1);
  }
  let key = std::env::var(KEY_VARIABLE).unwrap_or_default();
  if message.contains("Fail with the key") {
    eprintln!("the provider refused the key {key}");
    return ExitCode::from(1);
  }
  if message.contains("Say the key") {
    let text = format!("The key is {key}.");
    reply(session, "msg_key", &text);
    result(session, false, &text);
    return ExitCode::SUCCESS;
  }
  let text = match message.strip_prefix("Call ") {
    Some(call) => match server.as_ref().map(|server| server.call(call)) {
      Some(Ok(said)) => format!("Tool said: {said}"),
      Some(Err(error)) => format!("The call failed: {error}"),
      None => "No tools were configured.".to_owned(),
    },
    None => format!("Noted: {message}"),
  };
  reply(session, "msg_answer", &text);
  result(session, false, &text);
  ExitCode::SUCCESS
}

fn replay(session: &str, message: &str) {
  let line = json!({"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": message}]}, "session_id": session, "isReplay": true});
  println!("{line}");
}

fn reply(session: &str, id: &str, text: &str) {
  let block = json!({"type": "text", "text": text});
  println!(
    r#"{{"type":"assistant","message":{{"id":"{id}","role":"assistant","type":"message","content":[{block}],"usage":{USAGE}}},"parent_tool_use_id":null,"session_id":"{session}"}}"#
  );
  flush();
}

fn result(session: &str, is_error: bool, text: &str) {
  let line = json!({"type": "result", "subtype": if is_error { "error_during_execution" } else { "success" }, "is_error": is_error, "result": text, "session_id": session, "usage": serde_json::from_str::<Value>(USAGE).unwrap_or_default()});
  println!("{line}");
  flush();
}

fn flush() {
  if let Err(error) = std::io::stdout().flush() {
    eprintln!("cannot flush: {error}");
  }
}

fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a String> {
  args.iter().position(|arg| arg == flag).and_then(|at| args.get(at + 1))
}

fn server(args: &[String]) -> Option<ChatServer> {
  let config: Value = serde_json::from_str(&fs::read_to_string(after(args, "--mcp-config")?).ok()?).ok()?;
  let entry = config.pointer("/mcpServers/orchester")?;
  Some(ChatServer {
    url: entry.get("url")?.as_str()?.to_owned(),
    token: entry
      .pointer("/headers/Authorization")
      .and_then(Value::as_str)
      .map(str::to_owned),
  })
}

/// Appends how the fake was started to `.orchester/fake-chat.log`.
fn log(args: &[String], server: Option<&ChatServer>) {
  let prompt = after(args, "--system-prompt-file")
    .and_then(|path| fs::read_to_string(path).ok())
    .and_then(|text| text.lines().next().map(str::to_owned))
    .unwrap_or_default();
  let mut lines = vec![
    format!("argv: {}", args.join(" ")),
    format!("system-prompt: {prompt}"),
    format!("input-blocks: {}", crate::fake_claude_input::blocks()),
  ];
  if let Some(server) = server {
    lines.push(format!("mcp-url: {}", server.url));
    if server.token.is_some() {
      lines.push("mcp-token: present".to_owned());
    }
  }
  lines.extend(provider());
  let folder = Path::new(".orchester");
  let written = fs::create_dir_all(folder).and_then(|()| {
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

/// The log lines of the API provider the fake was started on, if any: never the key itself.
fn provider() -> Vec<String> {
  let Ok(url) = std::env::var("ANTHROPIC_BASE_URL") else {
    return Vec::new();
  };
  let models = [
    "ANTHROPIC_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "CLAUDE_CODE_SUBAGENT_MODEL",
  ]
  .map(|name| std::env::var(name).unwrap_or_default());
  let mut names: Vec<String> = std::env::vars()
    .map(|(name, _)| name)
    .filter(|name| name.to_ascii_uppercase().starts_with("ANTHROPIC_"))
    .collect();
  names.sort();
  let mut lines = vec![format!("provider: {url} {}", models.join(" "))];
  if std::env::var_os(KEY_VARIABLE).is_some() {
    lines.push("provider key: present".to_owned());
  }
  lines.push(format!("anthropic-env: {}", names.join(",")));
  lines
}
