//! The app's MCP server end to end on loopback: one server lists lurq's window tools and the app's
//! own, refuses requests without the bearer token, answers tool calls from the event loop, writes its
//! discovery file and removes it when it stops, and never shows the token anywhere else.

mod http;

use std::{path::PathBuf, thread, time::Duration};

use lurq::{
  app::{App, Tree},
  mcp::McpHandle,
};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  i18n,
  mcp::{self, McpSettings, McpStatus, WindowLink},
  root::{Root, RootProps},
  theme::{self, Mode},
};
use parking_lot::Mutex;
use serde_json::{Value, json};

use self::http::{Response, post};

/// Servers of one process share its discovery file, so these tests run one at a time.
static ONE_SERVER: Mutex<()> = Mutex::new(());

/// A window without a screen whose root tree runs the app's MCP server.
struct Served {
  app: App,
  tree: Tree,
  handle: McpHandle,
}

impl Served {
  fn start(window_control: bool) -> Self {
    let appearance = Appearance::new(ThemeSetting::Dark);
    let mut app = App::new();
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(1440, 900);
    let link = WindowLink::default();
    let settings = McpSettings::with_window_control(window_control);
    let handle = tree.enable_mcp(mcp::config(settings, &link, &appearance));
    let mcp = McpStatus::of(&handle);
    assert!(matches!(mcp, McpStatus::Listening { .. }), "the server listens");
    tree.mount_root::<Root>(&mut app, RootProps { link, mcp });
    tree.pass_headless(&mut app);
    Self { app, tree, handle }
  }

  fn port(&self) -> u16 {
    self.handle.port()
  }

  /// A new MCP session, initialized.
  fn session(&self) -> String {
    let token = Some(self.handle.token());
    let initialize = json!({
      "jsonrpc": "2.0", "id": 1, "method": "initialize",
      "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } },
    });
    let response = post(self.port(), token, None, &initialize);
    assert_eq!(response.status, 200, "{}", response.text);
    let session = response.session.expect("the server opens a session");
    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    post(self.port(), token, Some(&session), &initialized);
    session
  }

  fn tool_names(&self, session: &str) -> Vec<String> {
    let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} });
    let response = post(self.port(), Some(self.handle.token()), Some(session), &list);
    let message = response.message.expect("tools/list answers");
    message["result"]["tools"]
      .as_array()
      .expect("a list of tools")
      .iter()
      .filter_map(|tool| tool["name"].as_str().map(str::to_owned))
      .collect()
  }

  /// Calls `tool` and runs the event loop's part until the answer arrives.
  fn call(&mut self, session: &str, tool: &str, arguments: &Value) -> Response {
    let (port, token, session) = (self.port(), self.handle.token().to_owned(), session.to_owned());
    let body =
      json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": tool, "arguments": arguments } });
    let request = thread::spawn(move || post(port, Some(&token), Some(&session), &body));
    while !request.is_finished() {
      self.tree.drain_mcp_requests(&mut self.app);
      self.tree.pass_headless(&mut self.app);
      thread::sleep(Duration::from_millis(5));
    }
    request.join().expect("the request thread ends")
  }
}

impl Drop for Served {
  fn drop(&mut self) {
    self.tree.shutdown_mcp();
  }
}

/// The value a tool answered with, or the text of its error.
fn tool_value(response: &Response) -> (Value, bool) {
  let result = &response.message.as_ref().expect("the call answers")["result"];
  let text = result["content"][0]["text"].as_str().unwrap_or_default();
  let value = serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned()));
  (value, result["isError"].as_bool().unwrap_or(false))
}

/// This process's discovery file in lurq's per-user folder.
fn discovery_file() -> PathBuf {
  #[cfg(windows)]
  let base = PathBuf::from(std::env::var_os("LOCALAPPDATA").expect("LOCALAPPDATA is set"));
  #[cfg(target_os = "macos")]
  let base = PathBuf::from(std::env::var_os("HOME").expect("HOME is set"))
    .join("Library")
    .join("Application Support");
  base
    .join("lurq")
    .join("mcp")
    .join(format!("{}.json", std::process::id()))
}

#[test]
fn the_discovery_file_names_the_app_and_goes_with_the_server() {
  let _one = ONE_SERVER.lock();
  let served = Served::start(true);
  let contents: Value =
    serde_json::from_str(&std::fs::read_to_string(discovery_file()).expect("the discovery file is written"))
      .expect("the discovery file is JSON");
  assert_eq!(contents["app"], mcp::APP_NAME);
  assert_eq!(contents["port"], served.port());
  assert_eq!(contents["url"], format!("http://127.0.0.1:{}/mcp", served.port()));
  assert_eq!(
    contents["token"],
    served.handle.token(),
    "the token is in the file, for agents"
  );
  drop(served);
  assert!(!discovery_file().exists(), "the file goes when the server stops");
}

#[test]
fn requests_without_the_bearer_token_are_refused() {
  let _one = ONE_SERVER.lock();
  let served = Served::start(true);
  let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
  assert_eq!(post(served.port(), None, None, &ping).status, 401);
  assert_eq!(post(served.port(), Some("not-the-token"), None, &ping).status, 401);
}

#[test]
fn one_server_lists_lurqs_window_tools_and_the_apps_own() {
  let _one = ONE_SERVER.lock();
  let served = Served::start(true);
  let names = served.tool_names(&served.session());
  for tool in [
    "app_state",
    "navigate",
    "set_theme",
    "lurq_screenshot",
    "lurq_read_tree",
    "lurq_interact",
  ] {
    assert!(names.iter().any(|name| name == tool), "{tool} is listed: {names:?}");
  }
  assert!(
    !names.iter().any(|name| name == "lurq_navigate"),
    "only `navigate` moves the router"
  );
}

#[test]
fn without_window_control_agents_only_look() {
  let _one = ONE_SERVER.lock();
  let mut served = Served::start(false);
  let session = served.session();
  let names = served.tool_names(&session);
  assert!(names.iter().any(|name| name == "app_state") && names.iter().any(|name| name == "lurq_screenshot"));
  for tool in ["navigate", "set_theme", "lurq_interact"] {
    assert!(!names.iter().any(|name| name == tool), "{tool} is not listed");
  }
  let refused = served.call(&session, "navigate", &json!({ "route": "settings" }));
  assert!(
    !refused.text.contains("\"path\""),
    "navigate does not run: {}",
    refused.text
  );
}

#[test]
fn tools_answer_from_the_window_without_revealing_the_token() {
  let _one = ONE_SERVER.lock();
  let mut served = Served::start(true);
  let token = served.handle.token().to_owned();
  let session = served.session();
  let (state, failed) = tool_value(&served.call(&session, "app_state", &json!({})));
  assert!(!failed);
  assert_eq!(state["route"]["name"], "all_projects");
  assert_eq!(state["window"]["width"], 1440.0);
  let moved = served.call(&session, "navigate", &json!({ "route": "usage", "project": "kontur" }));
  assert_eq!(tool_value(&moved).0["route"]["path"], "/project/kontur/usage");
  let (error, failed) = tool_value(&served.call(&session, "navigate", &json!({ "route": "nowhere" })));
  assert!(failed);
  assert_eq!(error["code"], "unknown_route");
  let status = McpStatus::of(&served.handle);
  let status_bar = served.tree.get_element_by_id("status-mcp").map(|item| {
    item
      .children()
      .into_iter()
      .filter_map(|child| child.text_content().map(str::to_owned))
      .filter(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)))
      .collect::<Vec<_>>()
  });
  for shown in [
    state.to_string(),
    moved.text,
    format!("{status:?}"),
    format!("{status_bar:?}"),
  ] {
    assert!(!shown.contains(&token), "the token shows in {shown}");
  }
  assert_eq!(status_bar, Some(vec![format!("MCP on :{}", served.port())]));
}
