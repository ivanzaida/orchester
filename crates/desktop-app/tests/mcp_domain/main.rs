//! Orchester's domain tools on the app's MCP server, over loopback HTTP with the server's token: they
//! are listed beside lurq's window tools and the app's own, read and change the seeded project as its
//! screens do, tell the project's feed what they changed, and refuse what a person would be refused,
//! each with a stable code. `live` sets a project up end to end through the tools alone and works a
//! task with the real runner and the fake Codex. Needs `git`.

mod accounts;
mod accounts_window;
mod api_providers;
mod approve_everything;
mod automation_permissions;
mod base_moved;
mod basics;
mod board;
mod cloud_accounts;
mod completion;
mod connections_window;
mod decisions_reach_person;
mod delivery;
mod drafts;
mod gates;
#[path = "../mcp_server/http.rs"]
mod http;
mod instructions;
#[path = "../../examples/seed/kontur/mod.rs"]
#[allow(
  dead_code,
  reason = "the seed tests read all of what the seed reports; these tests read a part"
)]
mod kontur;
mod live;
mod mcp_servers;
mod merge;
mod merged_outside;
mod modules;
mod native_connections;
mod orchestrator_approval;
mod orchestrator_auto_merge;
mod orchestrator_chat;
mod orchestrator_contention;
mod orchestrator_continue;
mod orchestrator_continued;
mod orchestrator_git;
mod orchestrator_guard;
mod orchestrator_interrupted;
mod orchestrator_moves;
mod orchestrator_report_turn;
mod orchestrator_resolve;
mod orchestrator_sent;
mod orchestrator_sent_as;
mod person_decisions;
mod pipeline_permissions;
mod pull_requests;
mod reads;
mod reads_setup;
mod resolve;
mod return_ended;
#[path = "../../../workspace/tests/support/runner_programs.rs"]
mod runner_programs;
mod skills_window;
mod story_points;
mod task_organization;
mod tools;
mod tools_window;
mod unverified;
mod usage;
#[path = "../mcp_stable/vault.rs"]
mod vault;
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
  mcp::{self, AppMcp, McpLink, McpMode, McpSettings, WindowLink},
  project::{ProjectFeed, TasksState},
  root::{Root, RootProps},
  routes::{ProjectPage, Route, TaskList},
  services::Services,
  startup::Launch,
  test_support::signed_in,
  theme::{self, Mode},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::{secrets::Vault, tasks::resolve::Editor};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::runtime::Runtime;
use vault::MemoryVault;

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
    Self::start_on(folder, database, runner, (window_control, None))
  }

  /// [`Self::start_with`]; with a `vault`, the domain tools keep and read secret values there instead of in the
  /// keychain, as the session's services hand theirs to them.
  fn start_on(
    folder: TempDir,
    database: &Path,
    runner: PathBuf,
    (window_control, vault): (bool, Option<Arc<MemoryVault>>),
  ) -> Self {
    let runtime = Runtime::new().expect("tokio runtime");
    // An editor that is not one a person named, at a path where nothing is: *Resolve in editor* never opens one here.
    let launch = signed_in::launch(
      database,
      runner,
      runtime.handle(),
      &signed_in::account_cloud::AccountCloud::start(),
    )
    .with_editor(Some(Editor::VsCode(folder.path().join("no-editor").join("code"))));
    let mut services = launch.services().cloned();
    if let (Some(services), Some(vault)) = (services.as_mut(), vault) {
      services.vault = Vault::with(vault);
    }
    let mut app = App::new();
    app.set_tokio_handle(runtime.handle().clone());
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(1440, 900);
    let link = WindowLink::default();
    let appearance = Appearance::new(ThemeSetting::Dark, Arc::default(), None);
    let settings = McpSettings::with_window_control(window_control);
    let config = mcp::config(settings, &link, &appearance, services.as_ref(), app.i18n());
    let discovery = McpMode::Qa(folder.path().join("discovery"));
    let server = AppMcp::start(
      &mut tree,
      runtime.handle(),
      discovery,
      (config, settings),
      &link.input_gate(),
    );
    // These tests run in the server's process: use its handle, without reopening an ACL-protected discovery file.
    let token = server.token().expect("the server has a token").expose().to_owned();
    let port = server.port().expect("the server listens").get();
    let mcp = McpLink::new(&server);
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
    let (folder, seeded) = Self::seed(name, codex);
    let runner = folder.path().join("no-runner").join("orchester-runner");
    let database = seeded.app_database.clone();
    (Self::start(folder, &database, runner), seeded)
  }

  /// Scenario `name` served with its domain tools on `vault`.
  fn seeded_on_vault(name: &str, vault: Arc<MemoryVault>) -> (Self, Seeded) {
    let (folder, seeded) = Self::seed(name, None);
    let runner = folder.path().join("no-runner").join("orchester-runner");
    let database = seeded.app_database.clone();
    (Self::start_on(folder, &database, runner, (false, Some(vault))), seeded)
  }

  /// Scenario `name` seeded in a new folder.
  fn seed(name: &str, codex: Option<PathBuf>) -> (TempDir, Seeded) {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
    std::fs::create_dir_all(&scratch).expect("scratch");
    let folder = tempfile::tempdir_in(scratch).expect("a temporary folder");
    let request = SeedRequest {
      data_dir: folder.path().join("bin"),
      repo: folder.path().join(format!("repo-{name}")),
      scenario: scenarios::named(name).expect("a known scenario"),
      fake_codex: codex,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    (folder, seeded)
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
    if let Some(server) = self.server.take() {
      server.shutdown();
    }
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
