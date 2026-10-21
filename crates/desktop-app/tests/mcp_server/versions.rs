//! Which protocol revisions the servers speak: the app's own and the Orchestrator chat's per-turn one. Claude Code
//! opens with a `server/discover` probe for 2026-07-28 and takes that revision's stateless path when the answer
//! lists it; no Orchester server implements it, so each declines with the unsupported-version error (-32022) that
//! sends the client back to `initialize` on 2025-11-25.

use lurq::app::App;
use orchester_app::{i18n, mcp::chat::ChatServers, startup::Launch};
use orchester_core::id::{ChatId, ChatMessageId, ProjectId};
use orchester_workspace::{
  app_mcp,
  orchestrator::tools::{CHAT_TOOLS, ChatScope, TOOLS},
};
use serde_json::{Value, json};

use super::{Endpoint, Served, http::post, support::Data};

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

/// Asserts the server at `endpoint` declines `method` on the newer revision, naming only revisions up to
/// 2025-11-25.
fn assert_declined(endpoint: &Endpoint, method: &str) {
  let headers = [
    ("Authorization", endpoint.bearer.as_str()),
    ("Mcp-Method", method),
    ("Mcp-Protocol-Version", NEWER),
  ];
  let response = post(endpoint.port, &headers, &on_the_newer_revision("probe-1", method));
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

/// Asserts the server at `endpoint` declines the newer revision and that `initialize` asking for it agrees on
/// 2025-11-25.
fn assert_falls_back_to_initialize(endpoint: &Endpoint) {
  assert_declined(endpoint, "server/discover");
  assert_declined(endpoint, "tools/list");
  let initialize = json!({
    "jsonrpc": "2.0", "id": 1, "method": "initialize",
    "params": { "protocolVersion": NEWER, "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } },
  });
  let response = post(endpoint.port, &[("Authorization", &endpoint.bearer)], &initialize);
  let message = response.message.expect("initialize answers");
  assert_eq!(message["result"]["protocolVersion"], "2025-11-25", "{message}");
}

#[test]
fn the_newer_stateless_revision_is_declined_so_clients_fall_back_to_initialize() {
  let served = Served::start(true);
  assert_falls_back_to_initialize(&served.endpoint());
}

#[test]
fn clients_on_2025_11_25_list_the_tools() {
  let served = Served::start(true);
  let endpoint = served.endpoint();
  let names = endpoint.tool_names(&endpoint.session_on("2025-11-25"));
  assert!(names.iter().any(|name| name == "app_state"), "{names:?}");
}

/// The server of a turn of the Orchestrator chat on `launch`'s session, in `data`'s project folder, as its agent
/// reaches it; it serves while the returned servers live.
fn chat_endpoint(data: &Data, launch: &Launch) -> (Endpoint, ChatServers) {
  let services = launch.services().expect("the session starts");
  let app = App::new();
  i18n::install(&app).expect("bundled locales load");
  let scope = ChatScope {
    project: ProjectId::generate(),
    name: "Kontur".to_owned(),
    root: data.dir.path().to_path_buf(),
    chat: ChatId::generate(),
    on_its_own: false,
  };
  let servers = ChatServers::default();
  let tools = servers
    .serve_turn(services, app.i18n().clone(), (scope, ChatMessageId::generate()))
    .expect("the turn's tools are served");
  let port = tools
    .url
    .rsplit_once(':')
    .and_then(|(_, rest)| rest.trim_end_matches("/mcp").parse().ok())
    .unwrap_or_else(|| panic!("a loopback address with a port: {}", tools.url));
  let endpoint = Endpoint {
    port,
    bearer: format!("Bearer {}", tools.token.expose()),
  };
  (endpoint, servers)
}

#[test]
fn the_chat_server_declines_the_newer_revision_and_lists_its_tools_on_2025_11_25() {
  let data = Data::new();
  let launch = data.launch();
  let (endpoint, _servers) = chat_endpoint(&data, &launch);
  assert_falls_back_to_initialize(&endpoint);
  let names = endpoint.tool_names(&endpoint.session_on("2025-11-25"));
  assert!(names.iter().any(|name| name == "list_tasks"), "{names:?}");
}

/// What the chat's agent is served over the wire is every name of `TOOLS` the app has a tool for, `continue_run`
/// among them, and the chat's own (`CHAT_TOOLS`), and nothing else.
#[test]
fn the_chat_server_lists_every_chat_tool_the_app_has() {
  let data = Data::new();
  let launch = data.launch();
  let (endpoint, _servers) = chat_endpoint(&data, &launch);
  let mut served = endpoint.tool_names(&endpoint.session_on("2025-11-25"));
  let app: Vec<String> = app_mcp::listed()
    .into_iter()
    .chain(app_mcp::setup::listed())
    .map(|tool| tool.name.to_string())
    .collect();
  let mut expected: Vec<String> = TOOLS
    .iter()
    .filter(|name| app.iter().any(|tool| tool == *name))
    .chain(CHAT_TOOLS.iter())
    .map(|name| (*name).to_owned())
    .collect();
  served.sort();
  expected.sort();
  assert_eq!(served, expected, "the chat's tools/list");
  for name in [
    "continue_run",
    "merge_run",
    "hold_queue",
    "release_queue",
    "publish_pipeline",
    "init_repository",
  ] {
    assert!(served.iter().any(|tool| tool == name), "{name} is served: {served:?}");
  }
}
