//! Orchester's domain tools on the app's MCP server, over loopback HTTP with the server's token: they
//! are listed beside lurq's window tools and the app's own, read and change the seeded project as its
//! screens do, tell the project's feed what they changed, and refuse what a person would be refused,
//! each with a stable code. `live` sets a project up end to end through the tools alone and works a
//! task with the real runner and the fake Codex. Needs `git`.

mod gates;
#[path = "../mcp_server/http.rs"]
mod http;
#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed tests read all of what the seed reports; these tests read a part"
)]
mod kontur;
mod live;
mod mcp_servers;
mod merge;
mod window;

use std::{
  path::{Path, PathBuf},
  sync::Arc,
  thread,
  time::{Duration, Instant},
};

use http::post;
use kontur::{SeedRequest, Seeded, scenarios};
use lurq::app::{App, Tree};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  i18n,
  mcp::{self, AppMcp, DiscoveryFolder, McpSettings, McpStatus, WindowLink},
  project::{ProjectFeed, TasksState},
  root::{Root, RootProps},
  routes::{ProjectPage, Route, TaskList},
  services::Services,
  startup::Launch,
  theme::{self, Mode},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// How long a test waits for the runner and the fake agent.
const PATIENCE: Duration = Duration::from_secs(120);

/// The app's window headless on a session of a data folder of its own, with its MCP server listening
/// and one client session open. Window control is off: the domain tools need none.
struct Served {
  app: App,
  tree: Tree,
  link: WindowLink,
  server: Option<AppMcp>,
  port: u16,
  token: String,
  session: String,
  launch: Option<Launch>,
  _runtime: Runtime,
  folder: TempDir,
}

impl Served {
  /// The app on the application database `database` in `folder`, whose projects' runner is `runner`.
  fn start(folder: TempDir, database: &Path, runner: PathBuf) -> Self {
    Self::start_with(folder, database, runner, false)
  }

  /// [`Self::start`], with window control as given.
  fn start_with(folder: TempDir, database: &Path, runner: PathBuf, window_control: bool) -> Self {
    let runtime = Runtime::new().expect("tokio runtime");
    let launch = Launch::begin_at(database, runner, runtime.handle(), None);
    let mut app = App::new();
    app.set_tokio_handle(runtime.handle().clone());
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(1440, 900);
    let link = WindowLink::default();
    let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
    let settings = McpSettings::with_window_control(window_control);
    let config = mcp::config(settings, &link, &appearance, launch.services(), app.i18n());
    let discovery = DiscoveryFolder::At(folder.path().join("discovery"));
    let server =
      AppMcp::start(&mut tree, runtime.handle(), &discovery, config, &link.input_gate()).expect("the server listens");
    let file: Value = serde_json::from_str(&std::fs::read_to_string(server.discovery_path()).expect("discovery file"))
      .expect("the discovery file is JSON");
    let token = file["token"].as_str().expect("the file holds the token").to_owned();
    let port = server.port().get();
    let mcp = McpStatus::of(Some(&server), settings);
    tree.mount_root::<Root>(
      &mut app,
      RootProps {
        link: link.clone(),
        mcp,
        start: launch.start(),
      },
    );
    tree.pass_headless(&mut app);
    let mut served = Self {
      app,
      tree,
      link,
      server: Some(server),
      port,
      token,
      session: String::new(),
      launch: Some(launch),
      _runtime: runtime,
      folder,
    };
    served.session = served.open_session(&served.bearer());
    served
  }

  /// Scenario `name` seeded in a new folder, served with a runner program that does not exist.
  fn seeded(name: &str) -> (Self, Seeded) {
    Self::seeded_with(name, None)
  }

  /// Scenario `name` seeded with its Codex connection's program at `codex` (checked and signed in, as
  /// the seed records it), served with a runner program that does not exist.
  fn seeded_with(name: &str, codex: Option<PathBuf>) -> (Self, Seeded) {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let request = SeedRequest {
      data_dir: folder.path().join("bin"),
      repo: folder.path().join(format!("repo-{name}")),
      scenario: scenarios::named(name).expect("a known scenario"),
      fake_codex: codex,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let runner = folder.path().join("no-runner").join("orchester-runner");
    let database = seeded.app_database.clone();
    (Self::start(folder, &database, runner), seeded)
  }

  fn services(&self) -> &Services {
    self
      .launch
      .as_ref()
      .and_then(Launch::services)
      .expect("the session started")
  }

  fn bearer(&self) -> String {
    format!("Bearer {}", self.token)
  }

  /// A new MCP session, initialized, with `authorization` as its bearer.
  fn open_session(&self, authorization: &str) -> String {
    let initialize = json!({
      "jsonrpc": "2.0", "id": 1, "method": "initialize",
      "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } },
    });
    let response = post(self.port, &[("Authorization", authorization)], &initialize);
    assert_eq!(response.status, 200, "{}", response.text);
    let session = response.session.expect("the server opens a session");
    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    post(
      self.port,
      &[("Authorization", authorization), ("Mcp-Session-Id", &session)],
      &initialized,
    );
    session
  }

  fn tools(&self) -> Vec<Value> {
    let list = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} });
    let response = post(
      self.port,
      &[("Authorization", &self.bearer()), ("Mcp-Session-Id", &self.session)],
      &list,
    );
    let message = response.message.expect("tools/list answers");
    message["result"]["tools"].as_array().expect("a list of tools").clone()
  }

  /// Calls `tool` while the window runs: its value, or its error's `{code, message, reasons}`.
  fn call(&mut self, tool: &str, arguments: &Value) -> Result<Value, Value> {
    let (port, bearer, session) = (self.port, self.bearer(), self.session.clone());
    let body = json!({
      "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": tool, "arguments": arguments },
    });
    let request = thread::spawn(move || post(port, &[("Authorization", &bearer), ("Mcp-Session-Id", &session)], &body));
    while !request.is_finished() {
      self.pass();
      thread::sleep(Duration::from_millis(5));
    }
    let response = request.join().expect("the request thread ends");
    let result = &response
      .message
      .as_ref()
      .unwrap_or_else(|| panic!("{tool} answers: {}", response.text))["result"];
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    let value = serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned()));
    if result["isError"].as_bool().unwrap_or(false) {
      Err(value)
    } else {
      Ok(value)
    }
  }

  /// `tool`'s value; the test fails with its error.
  fn ok(&mut self, tool: &str, arguments: &Value) -> Value {
    self
      .call(tool, arguments)
      .unwrap_or_else(|error| panic!("{tool} refused: {error}"))
  }

  /// `tool`'s error; the test fails when it succeeds.
  fn refused(&mut self, tool: &str, arguments: &Value) -> Value {
    match self.call(tool, arguments) {
      Ok(value) => panic!("{tool} did what it should refuse: {value}"),
      Err(error) => error,
    }
  }

  /// Delivers what the window's futures finished, and lays it out.
  fn pass(&mut self) {
    self.tree.drain_mcp_requests(&mut self.app);
    self.tree.tick_futures();
    self.tree.pass_headless(&mut self.app);
  }

  /// Runs the window until `done` holds; fails with `what` after [`PATIENCE`].
  fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = Instant::now() + PATIENCE;
    while !done(self) {
      assert!(Instant::now() < deadline, "timed out waiting for {what}");
      self.pass();
      thread::sleep(Duration::from_millis(50));
    }
  }

  /// The feed of `project`, once the active tab shows its Tasks list.
  fn tasks_feed(&mut self, project: ProjectId) -> ProjectFeed {
    let navigation = self.link.navigation().expect("the window attached its navigation");
    navigation.go(Route::Project(project, ProjectPage::Tasks(TaskList::default())));
    self.until("the project to open", |served| {
      served.tree.get_element_by_id("screen-tasks").is_some()
    });
    self
      .link
      .projects()
      .expect("the window attached its projects")
      .feed(project)
  }
}

impl Drop for Served {
  fn drop(&mut self) {
    drop(self.server.take());
    if let Some(launch) = self.launch.take() {
      launch.end();
    }
  }
}

/// The titles of the tasks `feed` holds; `None` while it holds none.
fn titles(feed: &ProjectFeed) -> Option<Vec<String>> {
  match feed.tasks.get_untracked() {
    TasksState::Ready(tasks) => Some(
      tasks
        .tasks
        .iter()
        .map(|task| task.details().title.as_str().to_owned())
        .collect(),
    ),
    TasksState::Loading | TasksState::Failed(_) => None,
  }
}

#[test]
fn domain_tools_are_listed_beside_lurqs_and_the_apps_with_their_scopes() {
  let (served, _) = Served::seeded("empty");
  let tools = served.tools();
  let named = |name: &str| tools.iter().find(|tool| tool["name"] == name).cloned();
  for tool in [
    "app_state",
    "lurq_screenshot",
    "list_projects",
    "get_task",
    "run_task",
    "decide_approval",
    "add_project",
    "add_cli_connection",
    "add_mcp_server",
    "update_mcp_server",
    "remove_mcp_server",
    "set_mcp_server_enabled",
    "save_profile_draft",
    "save_pipeline_draft",
    "save_instruction_draft",
    "merge_run",
  ] {
    assert!(named(tool).is_some(), "{tool} is listed");
  }
  assert!(
    named("navigate").is_none(),
    "window control stays off while the data tools are on"
  );
  let read_only = |name: &str| named(name).expect("listed")["annotations"]["readOnlyHint"] == true;
  assert!(read_only("list_projects") && read_only("get_pipeline"));
  assert!(!read_only("create_task") && !read_only("merge_run"));
  let schema = &named("add_cli_connection").expect("listed")["inputSchema"];
  assert_eq!(schema["required"], json!(["project", "kind", "name"]));
}

#[test]
fn domain_tools_need_the_servers_token() {
  let (served, _) = Served::seeded("empty");
  let call = json!({
    "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": "list_projects", "arguments": {} },
  });
  let response = post(served.port, &[("Mcp-Session-Id", &served.session)], &call);
  assert_eq!(response.status, 401);
  let response = post(
    served.port,
    &[
      ("Authorization", "Bearer not-the-token"),
      ("Mcp-Session-Id", &served.session),
    ],
    &call,
  );
  assert_eq!(response.status, 401);
}

#[test]
fn domain_list_projects_tasks_and_a_task_read_the_seeded_project() {
  let (mut served, seeded) = Served::seeded("tasks");
  let projects = served.ok("list_projects", &json!({}));
  assert_eq!(projects["projects"][0]["name"], "Kontur");
  assert_eq!(projects["projects"][0]["id"], seeded.project.to_string());
  let tasks = served.ok("list_tasks", &json!({ "project": "Kontur" }));
  let keys: Vec<&str> = tasks["tasks"]
    .as_array()
    .expect("tasks")
    .iter()
    .filter_map(|task| task["key"].as_str())
    .collect();
  for task in &seeded.tasks {
    assert!(keys.contains(&task.key.as_str()), "{} is listed: {keys:?}", task.key);
  }
  let first = &seeded.tasks[0];
  let task = served.ok("get_task", &json!({ "project": "kontur", "key": first.key }));
  assert_eq!(task["title"], first.title);
  let missing = served.refused("get_task", &json!({ "project": "Kontur", "key": "KON-999" }));
  assert_eq!(missing["code"], "task_not_found");
  let unknown = served.refused("list_tasks", &json!({ "project": "Nowhere" }));
  assert_eq!(unknown["code"], "project_not_found");
}

#[test]
fn domain_create_task_makes_the_feed_read_again() {
  let (mut served, seeded) = Served::seeded("empty");
  let feed = served.tasks_feed(seeded.project);
  served.until("the first reading", |_| titles(&feed).is_some());
  let created = served.ok(
    "create_task",
    &json!({ "project": "Kontur", "title": "Retry with jitter", "brief": "Back off with jitter." }),
  );
  assert_eq!(created["task"]["submission"], "draft");
  served.until("the feed to read the new task", |_| {
    titles(&feed).is_some_and(|titles| titles.iter().any(|title| title == "Retry with jitter"))
  });
}

#[test]
fn domain_run_task_without_a_runner_is_a_typed_refusal() {
  // Any file stands in for the CLI: the seed records its check, and the runner never starts it.
  let codex = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
  let (mut served, _) = Served::seeded_with("empty", Some(codex));
  let task = json!({
    "project": "Kontur",
    "title": "Retry with jitter",
    "brief": "Back off with jitter.",
    "criteria": ["Retries are spread out."],
    "pipeline": "Quick fix",
  });
  let created = served.ok("create_task", &task);
  let key = created["task"]["key"].as_str().expect("a key").to_owned();
  let refused = served.refused("run_task", &json!({ "project": "Kontur", "key": key }));
  assert_eq!(refused["code"], "runner_unavailable", "{refused}");
  assert!(
    !refused["reasons"].as_array().expect("reasons").is_empty(),
    "the app's own words say why"
  );
}

#[test]
fn domain_automation_answers_approvals_only_while_a_person_lets_it() {
  let (mut served, _) = Served::seeded("empty");
  let refused = served.refused(
    "decide_approval",
    &json!({ "project": "Kontur", "approval": "any", "decision": "approve_once" }),
  );
  assert_eq!(refused["code"], "automation_not_allowed");
  let connection = served.ok("list_connections", &json!({ "project": "Kontur" }));
  assert!(connection["agents"].is_array());
}

#[test]
fn domain_setup_tools_check_their_input_as_the_forms_do() {
  let (mut served, seeded) = Served::seeded("empty");
  let folder = seeded.repo.display().to_string();
  let again = served.refused("add_project", &json!({ "name": "Kontur again", "folder": folder }));
  assert_eq!(again["code"], "already_initialized");
  let relative = served.refused("add_project", &json!({ "name": "Elsewhere", "folder": "repo" }));
  assert_eq!(relative["code"], "folder_not_absolute");
  let blank = served.refused("add_project", &json!({ "name": " ", "folder": folder }));
  assert_eq!(blank["code"], "invalid_name");
  let not_a_repository = served.folder.path().join("plain");
  std::fs::create_dir_all(&not_a_repository).expect("a plain folder");
  let plain = served.refused(
    "add_project",
    &json!({ "name": "Plain", "folder": not_a_repository.display().to_string() }),
  );
  assert_eq!(plain["code"], "not_a_git_repository");
  let unknown = served.refused(
    "save_profile_draft",
    &json!({ "project": "Kontur", "role": "Builder", "connection": "nowhere", "model": "gpt-5.5" }),
  );
  assert_eq!(unknown["code"], "connection_not_found");
  let environment = served.refused(
    "set_project_environment",
    &json!({ "project": "Kontur", "variables": [{ "name": "1BAD", "value": "x" }] }),
  );
  assert_eq!(environment["code"], "invalid_variable");
  let saved = served.ok(
    "set_project_environment",
    &json!({ "project": "Kontur", "variables": [{ "name": "RUST_LOG", "value": "info" }] }),
  );
  assert_eq!(saved["environment"], json!([{ "name": "RUST_LOG", "value": "info" }]));
  let files = served.ok(
    "set_repository_agent_files",
    &json!({ "project": "Kontur", "enabled": false }),
  );
  assert_eq!(files["repository_agent_files"], false);
  let project = served.ok("get_project", &json!({ "project": "Kontur" }));
  assert_eq!(project["project"]["key"], "KON");
  assert_eq!(project["project"]["repository_agent_files"], false);
  let unknown_argument = served.refused("get_project", &json!({ "project": "Kontur", "secret": "x" }));
  assert_eq!(unknown_argument["code"], "invalid_arguments");
}

#[test]
fn domain_pipelines_are_made_saved_and_refused_as_the_editor_does() {
  let (mut served, _) = Served::seeded("empty");
  let empty = served.ok(
    "create_pipeline",
    &json!({ "project": "Kontur", "name": "Hotfix", "from": "empty" }),
  );
  assert_eq!(empty["pipeline"]["state"], "draft_only");
  let taken = served.refused(
    "create_pipeline",
    &json!({ "project": "Kontur", "name": "hotfix", "from": "empty" }),
  );
  assert_eq!(taken["code"], "name_taken");
  let starter = served.ok(
    "create_pipeline",
    &json!({ "project": "Kontur", "name": "Delivery", "from": "starter" }),
  );
  let keys: Vec<&str> = starter["pipeline"]["steps"]
    .as_array()
    .expect("steps")
    .iter()
    .filter_map(|step| step["key"].as_str())
    .collect();
  assert_eq!(
    keys,
    [
      "research",
      "plan",
      "implement",
      "run-checks",
      "review",
      "create-pull-request"
    ]
  );
  let cycle = served.refused(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Hotfix", "steps": [
      { "key": "a", "name": "A", "runs_after": ["b"] },
      { "key": "b", "name": "B", "runs_after": ["a"] },
    ] }),
  );
  assert_eq!(cycle["code"], "invalid_pipeline", "{cycle}");
  let saved = served.ok(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Hotfix", "description": "Small fixes", "steps": [
      { "key": "fix", "name": "Fix", "budget": 50000 },
    ] }),
  );
  assert_eq!(saved["pipeline"]["steps"][0]["budget"], 50000);
  assert_eq!(saved["pipeline"]["description"], "Small fixes");
  let first = json!({ "project": "Kontur", "pipeline": "Hotfix" });
  assert_eq!(
    served.refused("publish_pipeline", &first)["code"],
    "person_only",
    "a pipeline's first version is a person's"
  );
  served.let_automation_widen(true);
  let unassigned = served.refused("publish_pipeline", &first);
  assert_eq!(unassigned["code"], "not_publishable");
  served.let_automation_widen(false);
  let write = served.refused(
    "save_pipeline_draft",
    &json!({ "project": "Kontur", "pipeline": "Hotfix", "steps": [
      { "key": "fix", "name": "Fix", "repository": "write" },
    ] }),
  );
  assert_eq!(
    write["code"], "invalid_step",
    "a step cannot write without a profile that writes"
  );
}
