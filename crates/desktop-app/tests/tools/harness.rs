use std::path::PathBuf;

use orchester_app::routes::{ProjectPage, Route, ToolTab};
use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{Tool, ToolAction},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::tools::{
  data::{ProjectTools, ToolEntry, create_tool},
  form::CommandText,
};

use super::support::{Window, project_id};
pub struct Tools {
  pub window: Window,
  pub repo: PathBuf,
}
impl Tools {
  pub fn new() -> Self {
    let window = Window::open();
    let repo = window.data.dir.path().join("repo-kontur");
    Self { window, repo }
  }
  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("project database")
  }
  pub fn go(&mut self, page: ProjectPage) {
    self.window.navigation().go(Route::Project(project_id(), page));
    self.window.pass();
    self.window.until("Tools content", |w| {
      w.tree.get_element_by_id("tools-library").is_some()
        || w.tree.get_element_by_id("tool-editor").is_some()
        || w.tree.get_element_by_id("tool-missing").is_some()
    });
  }
  pub fn edit(&mut self, id: ResourceId) {
    self.go(ProjectPage::Tool(id, ToolTab::Configuration));
    self.ready();
  }
  pub fn ready(&mut self) {
    self.window.until("program validation", |w| {
      !w.texts("tool-action-reason")
        .is_some_and(|t| t.join(" ").contains("Checking"))
    });
  }
  pub fn set(&mut self, id: &str, value: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|e| e.as_text_input())
      .unwrap_or_else(|| panic!("{id} input"))
      .set_value(value);
    self.window.pass();
  }
  pub fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} shown"))
      .click();
    self.window.pass();
  }
  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }
  pub fn text(&mut self, id: &str) -> String {
    self.window.texts(id).unwrap_or_else(|| panic!("{id} shown")).join(" ")
  }
  pub fn entry(&self, id: ResourceId) -> ToolEntry {
    ProjectTools::load(&self.db())
      .expect("tools read")
      .tool(id)
      .expect("tool exists")
      .clone()
  }
  pub fn seed(&self, name: &str, published: bool) -> ResourceId {
    let mut db = self.db();
    let tool = command();
    let id = create_tool(&mut db, &Name::new(name).expect("name"), &tool, Timestamp::now()).expect("tool created");
    if published {
      db.publish_draft::<Tool>(id, Timestamp::now()).expect("published");
    }
    id
  }
  pub fn confirm(&mut self) {
    self.window.until("question is armed", |w| {
      w.tree.tick_futures();
      w.pass();
      w.tree.get_element_by_id("tool-confirm-confirm").is_some()
    });
    self.click("tool-confirm-confirm");
  }
  pub fn wait_state(&mut self, state: &str) {
    self.window.until("editor state", |w| {
      w.texts("tool-state").is_some_and(|t| t.join(" ").contains(state))
    });
  }
}
pub fn command() -> Tool {
  CommandText {
    program: std::env::current_exe()
      .expect("test program")
      .to_string_lossy()
      .into_owned(),
    description: "Runs checks".into(),
    arguments: "test".into(),
    ..CommandText::new_tool()
  }
  .tool(None)
  .expect("command")
}
pub fn command_text(tool: &Tool) -> CommandText {
  assert!(matches!(tool.action(), ToolAction::Command(_)));
  CommandText::of(tool).expect("command fields")
}
