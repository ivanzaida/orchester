//! What `orchester-fake-claude` does with `--permission-prompt-tool`, as Claude Code does: it finds
//! the named tool's server among the `--mcp-config` files, starts it, speaks MCP to it on its
//! standard input and output, and calls the tool with `{tool_name, input, tool_use_id}` whenever it
//! wants to use a tool no rule allows. The decision is the `behavior` of the text the tool returns.

use std::{
  fs,
  io::{BufRead, BufReader, Write},
  process::{Child, ChildStdin, ChildStdout, Command, Stdio},
};

use serde_json::{Value, json};

/// A started permission prompt server.
pub struct Prompt {
  child: Child,
  input: ChildStdin,
  output: BufReader<ChildStdout>,
  tool: String,
  next: u64,
}

impl Prompt {
  /// The server of the `--permission-prompt-tool` in `args`, started and initialized; `None` when
  /// the CLI was not given one.
  pub fn open(args: &[String]) -> Option<Self> {
    let named = args
      .iter()
      .position(|arg| arg == "--permission-prompt-tool")
      .and_then(|index| args.get(index + 1))?;
    let mut parts = named.strip_prefix("mcp__")?.splitn(2, "__");
    let (server, tool) = (parts.next()?.to_owned(), parts.next()?.to_owned());
    let config = configs(args)
      .into_iter()
      .find_map(|config| config.pointer(&format!("/mcpServers/{server}")).cloned())?;
    let mut command = Command::new(config.get("command")?.as_str()?);
    for arg in config.get("args")?.as_array()? {
      command.arg(arg.as_str()?);
    }
    for (name, value) in config.get("env")?.as_object()? {
      command.env(name, value.as_str()?);
    }
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().ok()?;
    let input = child.stdin.take()?;
    let output = BufReader::new(child.stdout.take()?);
    let mut prompt = Self {
      child,
      input,
      output,
      tool,
      next: 0,
    };
    prompt.request(
      "initialize",
      &json!({"protocolVersion": "2025-06-18", "capabilities": {}}),
    )?;
    prompt.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))?;
    let tools = prompt.request("tools/list", &json!({}))?;
    let listed = tools["tools"]
      .as_array()?
      .iter()
      .any(|listed| listed["name"] == prompt.tool.as_str());
    listed.then_some(prompt)
  }

  /// The `behavior` the prompt decides for `tool` with `input` in call `call`; `None` when the
  /// server gave no usable answer.
  pub fn ask(&mut self, tool: &str, input: &Value, call: &str) -> Option<String> {
    let arguments = json!({"tool_name": tool, "input": input, "tool_use_id": call});
    let name = self.tool.clone();
    let result = self.request("tools/call", &json!({"name": name, "arguments": arguments}))?;
    let text = result.pointer("/content/0/text")?.as_str()?;
    let decision: Value = serde_json::from_str(text).ok()?;
    Some(decision.get("behavior")?.as_str()?.to_owned())
  }

  fn request(&mut self, method: &str, params: &Value) -> Option<Value> {
    self.next += 1;
    let id = self.next;
    self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
    let mut line = String::new();
    while self.output.read_line(&mut line).ok()? > 0 {
      let answer: Value = serde_json::from_str(&line).ok()?;
      if answer["id"] == id {
        return answer.get("result").cloned();
      }
      line.clear();
    }
    None
  }

  fn send(&mut self, message: &Value) -> Option<()> {
    writeln!(self.input, "{message}").ok()?;
    self.input.flush().ok()
  }
}

impl Drop for Prompt {
  fn drop(&mut self) {
    // A server that already ended with its input needs no ending.
    self.child.kill().ok();
  }
}

/// The `--mcp-config` files in `args`: every value after the flag up to the next option.
fn configs(args: &[String]) -> Vec<Value> {
  args
    .iter()
    .skip_while(|arg| *arg != "--mcp-config")
    .skip(1)
    .take_while(|arg| !arg.starts_with("--"))
    .filter_map(|path| fs::read_to_string(path).ok())
    .filter_map(|text| serde_json::from_str(&text).ok())
    .collect()
}

impl Prompt {
  /// Sends a call for `tool` without waiting for its answer, then ends the server once `release`
  /// exists: a relay that goes away while a person decides.
  pub fn leave_while_waiting(mut self, tool: &str, input: &Value, call: &str, release: &std::path::Path) {
    let arguments = json!({"tool_name": tool, "input": input, "tool_use_id": call});
    let name = self.tool.clone();
    self.next += 1;
    let request = json!({"jsonrpc": "2.0", "id": self.next, "method": "tools/call",
      "params": {"name": name, "arguments": arguments}});
    if self.send(&request).is_none() {
      eprintln!("cannot send the call");
    }
    while !release.exists() {
      std::thread::sleep(std::time::Duration::from_millis(50));
    }
  }
}

/// Connects to the runner end of the `--permission-prompt-tool` server the way its relay would,
/// showing `token` (the configured one when `None`) and one call; `answered` when the runner
/// answered it, else `closed`.
pub fn knock(args: &[String], token: Option<&str>) -> &'static str {
  let env = configs(args)
    .into_iter()
    .find_map(|config| config.pointer("/mcpServers/Orchester/env").cloned());
  let Some(env) = env else {
    return "no server";
  };
  let (Some(address), Some(configured)) = (
    env["ORCHESTER_APPROVAL_ADDRESS"].as_str(),
    env["ORCHESTER_APPROVAL_TOKEN"].as_str(),
  ) else {
    return "no server";
  };
  let Ok(mut stream) = std::net::TcpStream::connect(address) else {
    return "closed";
  };
  if stream
    .set_read_timeout(Some(std::time::Duration::from_secs(3)))
    .is_err()
  {
    return "closed";
  }
  let call = json!({"tool_name": "Bash", "input": {"command": "ls"}, "tool_use_id": "toolu_knock"});
  if writeln!(stream, "{}\n{call}", token.unwrap_or(configured)).is_err() {
    return "closed";
  }
  let mut answer = String::new();
  match BufReader::new(stream).read_line(&mut answer) {
    Ok(length) if length > 0 => "answered",
    _ => "closed",
  }
}
