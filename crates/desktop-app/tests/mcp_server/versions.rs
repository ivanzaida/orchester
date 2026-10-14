//! Which protocol revisions the server speaks. Claude Code opens with a `server/discover` probe for 2026-07-28 and
//! takes that revision's stateless path when the answer lists it; the server does not implement it, so it declines
//! with the unsupported-version error (-32022) that sends the client back to `initialize` on 2025-11-25.

use serde_json::{Value, json};

use super::{Served, http::post};

const UNSUPPORTED_PROTOCOL_VERSION: i64 = -32022;
const NEWER: &str = "2026-07-28";

/// A request as Claude Code 2.1.296 sends it on 2026-07-28: without a session, its version and client in `_meta`.
fn on_the_newer_revision(id: &str, method: &str) -> Value {
  json!({
    "jsonrpc": "2.0", "id": id, "method": method,
    "params": { "_meta": {
      "io.modelcontextprotocol/protocolVersion": NEWER,
      "io.modelcontextprotocol/clientInfo": { "name": "claude-code", "version": "2.1.296" },
      "io.modelcontextprotocol/clientCapabilities": { "roots": { "listChanged": true } },
    } },
  })
}

/// Asserts `served` declines `method` on the newer revision, naming only revisions up to 2025-11-25.
fn assert_declined(served: &Served, method: &str) {
  let headers = [
    ("Authorization", served.bearer()),
    ("Mcp-Method", method.to_owned()),
    ("Mcp-Protocol-Version", NEWER.to_owned()),
  ];
  let headers: Vec<(&str, &str)> = headers.iter().map(|(name, value)| (*name, value.as_str())).collect();
  let response = post(served.port, &headers, &on_the_newer_revision("probe-1", method));
  let message = response
    .message
    .unwrap_or_else(|| panic!("{method} answers: {}", response.text));
  assert!(
    message.get("result").is_none(),
    "{method} is not served on {NEWER}: {message}"
  );
  let error = &message["error"];
  assert_eq!(error["code"], UNSUPPORTED_PROTOCOL_VERSION, "{message}");
  let supported: Vec<&str> = error["data"]["supported"]
    .as_array()
    .expect("the error lists the supported revisions")
    .iter()
    .filter_map(Value::as_str)
    .collect();
  assert_eq!(supported.last(), Some(&"2025-11-25"), "{supported:?}");
  assert!(supported.iter().all(|version| *version < NEWER), "{supported:?}");
}

#[test]
fn the_newer_stateless_revision_is_declined_so_clients_fall_back_to_initialize() {
  let served = Served::start(true);
  assert_declined(&served, "server/discover");
  assert_declined(&served, "tools/list");
  let initialize = json!({
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": { "protocolVersion": NEWER, "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } },
  });
  let response = post(served.port, &[("Authorization", &served.bearer())], &initialize);
  let message = response.message.expect("initialize answers");
  assert_eq!(message["result"]["protocolVersion"], "2025-11-25", "{message}");
}

#[test]
fn clients_on_2025_11_25_list_the_tools() {
  let served = Served::start(true);
  let session = served.session_on("2025-11-25");
  let names = served.tool_names(&session);
  assert!(names.iter().any(|name| name == "app_state"), "{names:?}");
}
