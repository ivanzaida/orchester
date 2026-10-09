//! The app's own MCP server end to end on loopback: one server hosts lurq's window tools and the app's
//! own, refuses requests without its bearer token or from a web origin, answers tool calls from the
//! event loop, writes its discovery file, removes it when it stops and removes the files of processes
//! that ended, and never shows the token anywhere but the file. Each test uses a discovery folder of its
//! own, never the user's.

mod http;
#[path = "../support/mod.rs"]
mod support;

use std::{fs, path::Path, sync::Arc, thread, time::Duration};

use lurq::app::{App, Tree};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  i18n,
  mcp::{self, AppMcp, BearerToken, DiscoveryFolder, McpSettings, McpStatus, WindowLink},
  root::{Root, RootProps},
  startup::Launch,
  theme::{self, Mode},
};
use serde_json::{Value, json};
use tempfile::TempDir;

use self::{
  http::{Response, post},
  support::Data,
};

/// A window without a screen whose root tree hosts the tools of the app's MCP server.
struct Served {
  app: App,
  tree: Tree,
  server: Option<AppMcp>,
  folder: TempDir,
  port: u16,
  token: String,
  _launch: Launch,
  _data: Data,
}

impl Served {
  fn start(window_control: bool) -> Self {
    Self::start_in(tempfile::tempdir().expect("a temporary folder"), window_control)
  }

  fn start_in(folder: TempDir, window_control: bool) -> Self {
    let data = Data::new();
    let launch = data.launch();
    let runtime = data.runtime.handle();
    let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
    let mut app = App::new();
    app.set_tokio_handle(runtime.clone());
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(1440, 900);
    let link = WindowLink::default();
    let config = mcp::config(
      McpSettings::with_window_control(window_control),
      &link,
      &appearance,
      launch.services(),
      app.i18n(),
    );
    let at = DiscoveryFolder::At(folder.path().to_path_buf());
    let server = AppMcp::start(&mut tree, runtime, &at, config, &link.input_gate()).expect("the server listens");
    let discovery = read_json(server.discovery_path());
    let token = discovery["token"]
      .as_str()
      .expect("the file holds the token")
      .to_owned();
    let port = server.port().get();
    let mcp = McpStatus::of(Some(&server), McpSettings::with_window_control(window_control));
    let start = launch.start();
    tree.mount_root::<Root>(&mut app, RootProps { link, mcp, start });
    tree.pass_headless(&mut app);
    Self {
      app,
      tree,
      server: Some(server),
      folder,
      port,
      token,
      _launch: launch,
      _data: data,
    }
  }

  fn bearer(&self) -> String {
    format!("Bearer {}", self.token)
  }

  /// A new MCP session, initialized.
  fn session(&self) -> String {
    let initialize = json!({
      "jsonrpc": "2.0", "id": 1, "method": "initialize",
      "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } },
    });
    let response = post(self.port, &[("Authorization", &self.bearer())], &initialize);
    assert_eq!(response.status, 200, "{}", response.text);
    let session = response.session.expect("the server opens a session");
    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    post(
      self.port,
      &[("Authorization", &self.bearer()), ("Mcp-Session-Id", &session)],
      &initialized,
    );
    session
  }

  fn tool_names(&self, session: &str) -> Vec<String> {
    let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} });
    let response = post(
      self.port,
      &[("Authorization", &self.bearer()), ("Mcp-Session-Id", session)],
      &list,
    );
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
    let (port, bearer, session) = (self.port, self.bearer(), session.to_owned());
    let body =
      json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": tool, "arguments": arguments } });
    let request = thread::spawn(move || post(port, &[("Authorization", &bearer), ("Mcp-Session-Id", &session)], &body));
    while !request.is_finished() {
      self.tree.drain_mcp_requests(&mut self.app);
      self.tree.pass_headless(&mut self.app);
      thread::sleep(Duration::from_millis(5));
    }
    request.join().expect("the request thread ends")
  }
}

fn read_json(path: &Path) -> Value {
  serde_json::from_str(&fs::read_to_string(path).expect("the file is readable")).expect("the file is JSON")
}

/// The value a tool answered with, or the text of its error.
fn tool_value(response: &Response) -> (Value, bool) {
  let result = &response.message.as_ref().expect("the call answers")["result"];
  let text = result["content"][0]["text"].as_str().unwrap_or_default();
  let value = serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned()));
  (value, result["isError"].as_bool().unwrap_or(false))
}

#[test]
fn the_discovery_file_names_the_app_and_goes_with_the_server() {
  let mut served = Served::start(true);
  let server = served.server.take().expect("the server runs");
  let file = server.discovery_path().to_path_buf();
  assert_eq!(file, served.folder.path().join(format!("{}.json", std::process::id())));
  let contents = read_json(&file);
  assert_eq!(contents["app"], mcp::APP_NAME);
  assert_eq!(contents["port"], served.port);
  assert_eq!(contents["url"], format!("http://127.0.0.1:{}/mcp", served.port));
  assert_eq!(served.token.len(), 64, "two random UUIDs");
  assert!(
    file.with_extension("lock").exists(),
    "the lock marks the process as alive"
  );
  drop(server);
  assert!(
    !file.exists() && !file.with_extension("lock").exists(),
    "both go when the server stops"
  );
}

#[test]
fn files_of_processes_that_ended_go_when_a_server_starts() {
  let folder = tempfile::tempdir().expect("a temporary folder");
  let ended = folder.path().join("999999.json");
  let foreign = folder.path().join("notes.json");
  fs::write(&ended, "{}").expect("an ended process's file");
  fs::write(&foreign, "{}").expect("another file");
  let served = Served::start_in(folder, true);
  assert!(!ended.exists(), "a file without a held lock is stale");
  assert!(foreign.exists(), "files not named after a pid stay");
  drop(served);
}

#[test]
fn requests_without_the_token_or_from_a_web_origin_are_refused() {
  let served = Served::start(true);
  let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
  assert_eq!(post(served.port, &[], &ping).status, 401);
  assert_eq!(
    post(served.port, &[("Authorization", "Bearer not-the-token")], &ping).status,
    401
  );
  let from_web = [
    ("Authorization", served.bearer()),
    ("Origin", "https://example.com".to_owned()),
  ];
  let from_web: Vec<(&str, &str)> = from_web.iter().map(|(name, value)| (*name, value.as_str())).collect();
  assert_eq!(post(served.port, &from_web, &ping).status, 403);
}

#[test]
fn one_server_hosts_lurqs_window_tools_and_the_apps_own() {
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
  let mut served = Served::start(false);
  let session = served.session();
  let names = served.tool_names(&session);
  assert!(names.iter().any(|name| name == "app_state") && names.iter().any(|name| name == "lurq_screenshot"));
  for tool in ["navigate", "set_theme", "lurq_interact"] {
    assert!(!names.iter().any(|name| name == tool), "{tool} is not listed");
  }
  let refused = served.call(&session, "navigate", &json!({ "route": "settings" }));
  assert!(refused.text.contains("unknown tool: navigate"), "{}", refused.text);
  let (state, _) = tool_value(&served.call(&session, "app_state", &json!({})));
  assert_eq!(state["route"]["path"], "/", "the window stays where it was");
}

#[test]
fn tools_answer_from_the_window_without_revealing_the_token() {
  let mut served = Served::start(true);
  let session = served.session();
  let (state, failed) = tool_value(&served.call(&session, "app_state", &json!({})));
  assert!(!failed);
  assert_eq!(state["route"]["name"], "all_projects");
  assert_eq!(state["window"]["width"], 1440.0);
  let moved = served.call(
    &session,
    "navigate",
    &json!({ "route": "usage", "project": "3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b" }),
  );
  assert_eq!(
    tool_value(&moved).0["route"]["path"],
    "/project/3c7d2f5e-8b1a-4e6f-9a2b-0c4d5e6f7a8b/usage"
  );
  let (error, failed) = tool_value(&served.call(&session, "navigate", &json!({ "route": "nowhere" })));
  assert!(failed);
  assert_eq!(error["code"], "unknown_route");
  let status = McpStatus::of(served.server.as_ref(), McpSettings::with_window_control(true));
  let status_bar = served.tree.get_element_by_id("status-mcp").map(|item| {
    item
      .children()
      .into_iter()
      .filter_map(|child| child.text_content().map(str::to_owned))
      .filter(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)))
      .collect::<Vec<_>>()
  });
  let token = served.token.clone();
  let shown = [
    state.to_string(),
    moved.text,
    format!("{status:?}"),
    format!("{status_bar:?}"),
  ];
  for text in shown {
    assert!(!text.contains(&token), "the token shows in {text}");
  }
  assert_eq!(format!("{:?}", BearerToken::generate()), "BearerToken(<redacted>)");
  assert_eq!(status_bar, Some(vec![format!("MCP on :{}", served.port)]));
  assert!(
    served.tree.get_element_by_id("status-window-control").is_some(),
    "the status bar says agents can control the window"
  );
}
