//! The MCP side of the permission prompt relay: newline-delimited JSON-RPC on the relay's standard
//! input and output, as Claude Code speaks it to a `stdio` server. The relay answers the handshake
//! and the tool list itself and hands each call of [`super::TOOL`] to the runner; every other
//! request is answered with an error, so the CLI never waits on one.

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
  /// A notification or a line that is not a request: nothing to answer.
  Ignore,
}

/// The step for `line`.
pub fn relay_step(line: &str) -> RelayStep {
  let Ok(message) = serde_json::from_str::<Value>(line) else {
    return RelayStep::Ignore;
  };
  let (Some(id), Some(method)) = (message.get("id"), message.get("method").and_then(Value::as_str)) else {
    return RelayStep::Ignore;
  };
  let params = message.get("params").unwrap_or(&Value::Null);
  let result = match method {
    "initialize" => json!({
      "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
      "capabilities": {"tools": {}},
      "serverInfo": {"name": "orchester", "version": env!("CARGO_PKG_VERSION")},
    }),
    "tools/list" => json!({"tools": [{
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
    }]}),
    "tools/call" if params.get("name").and_then(Value::as_str) == Some(TOOL) => {
      return RelayStep::Ask {
        id: id.clone(),
        arguments: params.get("arguments").cloned().unwrap_or(Value::Null),
      };
    }
    "tools/call" => json!({"content": [{"type": "text", "text": "Unknown tool."}], "isError": true}),
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

/// The answer to call `id`: the permission prompt's decision text ([`super::prompt_result`]).
pub fn ask_result(id: &Value, decision: &str) -> String {
  json!({"jsonrpc": "2.0", "id": id, "result": {"content": [{"type": "text", "text": decision}]}}).to_string()
}
