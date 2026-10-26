use std::{path::PathBuf, time::Duration};

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
    invocation::Platform,
  },
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::AgentPermissions,
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::agents::data::{self, ProjectAgents};

use super::support::{Data, Window, project_id};

pub struct Agents {
  pub window: Window,
  pub root: PathBuf,
}
impl Agents {
  pub fn new(connection: bool) -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/agents");
    std::fs::create_dir_all(&scratch).expect("scratch directory");
    let data = Data::at(
      tempfile::tempdir_in(scratch).expect("test directory"),
      tokio::runtime::Runtime::new().expect("runtime"),
    );
    let launch = data.launch();
    let project = data.add_kontur(launch.workspace().expect("workspace"));
    let root = project.repo_root;
    if connection {
      let mut db = ProjectDb::open(&root).expect("database");
      db.create_connection(&codex(&root)).expect("connection");
    }
    Self {
      window: Window::of(data, launch),
      root,
    }
  }
  pub fn profile(&self, published: bool) -> ResourceId {
    let listed = ProjectAgents::load_in(&self.root).expect("agents");
    let c = &listed.connections[0];
    let profile = AgentProfile::new(
      Name::new("Builder").expect("name"),
      "Builds the change".into(),
      PromptText::new("Implement the approved plan.").expect("instructions"),
      c.id(),
      ModelId::new("test-model").expect("model"),
      ProviderSettings::default(),
    )
    .with_permissions(AgentPermissions::most_restrictive(c.provider(), Platform::current()));
    let mut db = ProjectDb::open(&self.root).expect("database");
    let id = data::create_profile(&mut db, &profile, Timestamp::now()).expect("profile");
    if published {
      data::publish(&mut db, id, Timestamp::now()).expect("publish");
    }
    id
  }
  pub fn go(&mut self) {
    self
      .window
      .navigation()
      .go(Route::Project(project_id(), ProjectPage::Agents));
    self.until("agents", |s| s.exists("agents-new"));
  }
  pub fn open(&mut self, id: ResourceId) {
    self.go();
    self.click(&format!("agent-{id}"));
    self.until("editor", |s| s.exists("agent-role"));
  }
  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(self) {
        return;
      }
      assert!(std::time::Instant::now() < deadline, "timed out: {what}");
      std::thread::sleep(Duration::from_millis(5));
    }
  }
  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }
  pub fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} exists"))
      .click();
    self.window.pass();
  }
  /// Presses `id` until `done`: a button that waits on a read or a write takes the press once it is ready.
  pub fn press_until(&mut self, id: &str, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    self.until(what, |s| {
      if let Some(mut button) = s.window.tree.get_element_by_id_mut(id) {
        button.click();
      }
      s.window.pass();
      done(s)
    });
  }
  /// What the person typed into the input `id`.
  pub fn value(&mut self, id: &str) -> String {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.as_text_input())
      .unwrap_or_else(|| panic!("{id} input"))
      .value()
  }
  /// Waits until the edits so far are saved as the draft: the profile's page saves on its own once they rest.
  pub fn saved(&mut self) {
    self.until("the draft saved on its own", |s| {
      s.exists("agent-save-status") && {
        let status = s.text("agent-save-status");
        !status.contains("Unsaved") && !status.contains("Saving")
      } && !s.exists("agent-saving")
    });
  }
  pub fn set(&mut self, id: &str, text: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.as_text_input())
      .unwrap_or_else(|| panic!("{id} input"))
      .set_value(text);
    self.window.pass();
  }
  pub fn text(&mut self, id: &str) -> String {
    self.window.texts(id).unwrap_or_else(|| panic!("{id} exists")).join(" ")
  }
  pub fn listed(&self) -> ProjectAgents {
    ProjectAgents::load_in(&self.root).expect("agents")
  }
}
pub fn changed(profile: &AgentProfile) -> AgentProfile {
  AgentProfile::new(
    profile.role().clone(),
    "Changed elsewhere".into(),
    profile.instructions().clone(),
    profile.connection(),
    profile.model().clone(),
    profile.settings().clone(),
  )
  .with_permissions(profile.permissions().clone())
  .with_mcp_servers(profile.mcp_servers().clone())
}
fn codex(root: &std::path::Path) -> AgentConnection {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Test Codex").expect("name"),
    AgentProvider::CodexCli,
    root.join("codex"),
    Timestamp::now(),
  )
  .expect("absolute program");
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: "0.139.0".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(
          CatalogSource::Provider,
          vec![CatalogModel::new(
            ModelId::new("test-model").expect("model"),
            vec!["low".into(), "high".into()],
          )],
        ),
      },
    })
    .expect("check");
  connection
}
