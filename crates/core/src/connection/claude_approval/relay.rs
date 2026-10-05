//! The MCP side of the relay the runner serves each invocation through: newline-delimited JSON-RPC on
//! the relay's standard input and output, as Claude Code and Codex speak it to a `stdio` server. The relay
//! answers the handshake itself and hands everything else to the runner over its loopback connection, one
//! [`RelayRequest`] per line: a call of [`super::TOOL`] (Claude Code's permission prompt, served only when
//! the relay is started as that prompt), the list of the step's Orchester tools
//! (`crate::orchester_tools`), and a call of one of them. Every other request is answered with an error,
//! so the CLI never waits on one.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::TOOL;

/// Used when the CLI does not say which protocol version it speaks.
const PROTOCOL_VERSION: &str = "2025-06-18";
const METHOD_NOT_FOUND: i64 = -32601;

/// What the relay does with one line from the CLI.
#[derive(Clone, Debug, PartialEq)]
pub enum RelayStep {
  /// Write this line back.
  Reply(String),
  /// Ask the runner about the call with these arguments, then answer request `id` with
  /// [`ask_result`].
  Ask { id: Value, arguments: Value },
  /// Ask the runner for the step's tools, then answer request `id` with [`list_result`].
  List { id: Value },
  /// Hand the call of tool `name` to the runner, then answer request `id` with [`call_result`].
  Call { id: Value, name: String, arguments: Value },
  /// A notification or a line that is not a request: nothing to answer.
  Ignore,
}

/// What the relay asks the runner, one JSON line each; the runner answers each on one line: an [`Ask`]
/// with the permission prompt's decision text, a [`List`] with a JSON array of MCP tools, a [`Call`]
/// with an MCP `tools/call` result.
///
/// [`Ask`]: RelayRequest::Ask
/// [`List`]: RelayRequest::List
/// [`Call`]: RelayRequest::Call
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayRequest {
  Ask(Value),
  List,
  Call { name: String, arguments: Value },
}

/// The step for `line`; `prompt` when the relay serves Claude Code's permission prompt.
pub fn relay_step(line: &str, prompt: bool) -> RelayStep {
  let Ok(message) = serde_json::from_str::<Value>(line) else {
    return RelayStep::Ignore;
  };
  let (Some(id), Some(method)) = (message.get("id"), message.get("method").and_then(Value::as_str)) else {
    return RelayStep::Ignore;
  };
  let params = message.get("params").unwrap_or(&Value::Null);
  let arguments = || params.get("arguments").cloned().unwrap_or(Value::Null);
  let result = match method {
    "initialize" => json!({
      "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
      "capabilities": {"tools": {}},
      "serverInfo": {"name": "orchester", "version": env!("CARGO_PKG_VERSION")},
    }),
    "tools/list" => return RelayStep::List { id: id.clone() },
    "tools/call" => {
      let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
      if prompt && name == TOOL {
        return RelayStep::Ask {
          id: id.clone(),
          arguments: arguments(),
        };
      }
      return RelayStep::Call {
        id: id.clone(),
        name: name.to_owned(),
        arguments: arguments(),
      };
    }
    "ping" => json!({}),
    _ => {
      return RelayStep::Reply(
        json!({"jsonrpc": "2.0", "id": id, "error": {"code": METHOD_NOT_FOUND, "message": "Method not found"}})
          .to_string(),
      );
    }
  };
  RelayStep::Reply(json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string())
}

/// The answer to permission prompt call `id`: the decision text ([`super::prompt_result`]).
pub fn ask_result(id: &Value, decision: &str) -> String {
  call_result(id, &json!({"content": [{"type": "text", "text": decision}]}))
}

/// The answer to `tools/list` request `id`: `tools`, after the permission prompt's own tool when the
/// relay serves it (`prompt`).
pub fn list_result(id: &Value, tools: Vec<Value>, prompt: bool) -> String {
  let mut listed = Vec::with_capacity(tools.len() + 1);
  if prompt {
    listed.push(approve_tool());
  }
  listed.extend(tools);
  json!({"jsonrpc": "2.0", "id": id, "result": {"tools": listed}}).to_string()
}

/// The answer to tool call `id`: `result`, an MCP `tools/call` result.
pub fn call_result(id: &Value, result: &Value) -> String {
  json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string()
}

fn approve_tool() -> Value {
  json!({
    "name": TOOL,
    "description": "Asks a person in Orchester to approve one tool call.",
    "inputSchema": {
      "type": "object",
      "properties": {
        "tool_name": {"type": "string"},
        "input": {"type": "object"},
        "tool_use_id": {"type": "string"},
      },
      "required": ["tool_name", "input"],
    },
  })
}
