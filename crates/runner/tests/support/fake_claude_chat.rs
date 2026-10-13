//! `orchester-fake-claude` as a turn of the Orchestrator chat: started with `--system-prompt-file` and an
//! `--mcp-config` of the chat's domain tools. It appends how it was started to `.orchester/fake-chat.log` in its
//! working directory (`argv: `, `system-prompt: ` with the file's first line, `mcp-url: `, and `mcp-token: present`
//! when the configuration carries a bearer token), then answers by the message:
//!
//! - "Call <tool> <JSON arguments>": calls that tool of the configured server over streamable HTTP with the bearer
//!   token and replies with what it answered (`Tool said: <result text>`);
//! - "Keep thinking": writes the start of a reply and waits until it is ended;
//! - "Fail now": ends with an error result and exit code 1;
//! - anything else: replies `Noted: <message>`.
//!
//! Each answer ends with a result that reports usage, as Claude Code does.

#![allow(dead_code, reason = "each fake CLI uses a part of the shared support")]

use std::{fs, io::Write, path::Path, process::ExitCode, thread, time::Duration};

use serde_json::{Value, json};

const USAGE: &str = r#"{"input_tokens":12,"output_tokens":27}"#;

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

/// The configured server: its address and bearer token.
struct Server {
  url: String,
  token: Option<String>,
}

fn after<'a>(args: &'a [String], flag: &str) -> Option<&'a String> {
  args.iter().position(|arg| arg == flag).and_then(|at| args.get(at + 1))
}

fn server(args: &[String]) -> Option<Server> {
  let config: Value = serde_json::from_str(&fs::read_to_string(after(args, "--mcp-config")?).ok()?).ok()?;
  let entry = config.pointer("/mcpServers/orchester")?;
  Some(Server {
    url: entry.get("url")?.as_str()?.to_owned(),
    token: entry
      .pointer("/headers/Authorization")
      .and_then(Value::as_str)
      .map(str::to_owned),
  })
}

/// Appends how the fake was started to `.orchester/fake-chat.log`.
fn log(args: &[String], server: Option<&Server>) {
  let prompt = after(args, "--system-prompt-file")
    .and_then(|path| fs::read_to_string(path).ok())
    .and_then(|text| text.lines().next().map(str::to_owned))
    .unwrap_or_default();
  let mut lines = vec![format!("argv: {}", args.join(" ")), format!("system-prompt: {prompt}")];
  if let Some(server) = server {
    lines.push(format!("mcp-url: {}", server.url));
    if server.token.is_some() {
      lines.push("mcp-token: present".to_owned());
    }
  }
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

impl Server {
  /// Calls the tool `call` names (`<tool> <JSON arguments>`) and returns the text of its result.
  fn call(&self, call: &str) -> Result<String, String> {
    let (tool, arguments) = call.split_once(' ').unwrap_or((call, "{}"));
    let arguments: Value = serde_json::from_str(arguments).map_err(|error| error.to_string())?;
    let client = reqwest::blocking::Client::new();
    let initialize = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "fake-claude", "version": "1"}}});
    let (session, _) = self.post(&client, None, &initialize)?;
    let initialized = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    self.post(&client, session.as_deref(), &initialized)?;
    let called =
      json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": tool, "arguments": arguments}});
    let (_, answer) = self.post(&client, session.as_deref(), &called)?;
    let answer = answer.ok_or("no answer to the call")?;
    let text = answer
      .pointer("/result/content/0/text")
      .and_then(Value::as_str)
      .map_or_else(|| answer.to_string(), str::to_owned);
    Ok(text)
  }

  /// Posts `body`: the session the server named and the JSON-RPC message it answered with, if any.
  fn post(
    &self,
    client: &reqwest::blocking::Client,
    session: Option<&str>,
    body: &Value,
  ) -> Result<(Option<String>, Option<Value>), String> {
    let mut request = client
      .post(&self.url)
      .header("Content-Type", "application/json")
      .header("Accept", "application/json, text/event-stream")
      .body(body.to_string());
    if let Some(token) = &self.token {
      request = request.header("Authorization", token);
    }
    if let Some(session) = session {
      request = request.header("Mcp-Session-Id", session);
    }
    let response = request.send().map_err(|error| error.to_string())?;
    let status = response.status();
    let session = response
      .headers()
      .get("mcp-session-id")
      .and_then(|value| value.to_str().ok())
      .map(str::to_owned);
    let text = response.text().map_err(|error| error.to_string())?;
    if !status.is_success() {
      return Err(format!("HTTP {status}"));
    }
    let message = text
      .lines()
      .filter_map(|line| line.strip_prefix("data:"))
      .chain(std::iter::once(text.as_str()))
      .find_map(|data| {
        serde_json::from_str::<Value>(data.trim())
          .ok()
          .filter(|value| value.get("id").is_some())
      });
    Ok((session, message))
  }
}
