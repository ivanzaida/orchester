//! A window on a seeded Kontur (`examples/seed`, the `connections` scenario unless a test seeds another),
//! driven by element id as a person would, and what Connections stored, read back from the project's
//! database.

use std::{path::PathBuf, time::Duration};

use orchester_app::routes::{ConnectionsTab, ConnectionsView, NewConnection, ProjectPage, Route};
use orchester_core::{
  connection::mcp::McpServer,
  id::{ConnectionId, ProjectId},
  store::McpServerStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window},
};

/// Room for the list and the whole inspector.
const SIZE: (u32, u32) = (1440, 1300);

pub struct Seeded {
  pub window: Window,
  pub project: ProjectId,
  pub repo: PathBuf,
}

impl Seeded {
  pub fn new(scenario: &str) -> Self {
    let data = Data::new();
    let seeded = kontur::seed(&SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: data.dir.path().join(format!("repo-{scenario}")),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    })
    .expect("the scenario seeds");
    let launch = data.launch();
    let mut window = Window::of(data, launch);
    window.tree.resize(SIZE.0, SIZE.1);
    Self {
      window,
      project: seeded.project,
      repo: seeded.repo,
    }
  }

  pub fn go(&mut self, page: ProjectPage) {
    self.window.navigation().go(Route::Project(self.project, page));
    self.window.pass();
  }

  /// Connections on `tab`, once the connections are read.
  pub fn list(&mut self, tab: ConnectionsTab) {
    self.go(ProjectPage::Connections(ConnectionsView::new(tab)));
    self.until("the connections", |seeded| {
      seeded.exists("connections-body") || seeded.exists("connections-empty") || seeded.exists("connections-empty-tab")
    });
  }

  /// Add connection or Add MCP server, once its fields are shown.
  pub fn add(&mut self, kind: NewConnection) {
    self.go(ProjectPage::NewConnection(kind));
    self.until("the form", |seeded| seeded.exists("connection-form"));
  }

  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(self) {
        return;
      }
      assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
      std::thread::sleep(Duration::from_millis(5));
    }
  }

  pub fn set(&mut self, id: &str, value: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .set_value(value);
    self.window.pass();
  }

  pub fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .click();
    self.window.pass();
  }

  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// The words of the element `id`, joined.
  pub fn text(&mut self, id: &str) -> String {
    self
      .window
      .texts(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .join(" ")
  }

  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("the project's database opens")
  }

  pub fn server(&self, name: &str) -> Option<McpServer> {
    self
      .db()
      .mcp_servers()
      .expect("the servers read")
      .into_iter()
      .find(|server| server.name().as_str() == name)
  }

  /// The connection the route's inspector shows.
  pub fn selected(&self) -> Option<ConnectionId> {
    match self.window.navigation().route_now() {
      Route::Project(_, ProjectPage::Connections(view)) => view.selected(),
      _ => None,
    }
  }

  /// The row of connection `id`.
  pub fn row(id: ConnectionId) -> String {
    format!("connection-row-{id}")
  }
}
