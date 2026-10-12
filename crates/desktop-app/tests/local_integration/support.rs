use std::{
  fs,
  path::{Path, PathBuf},
  sync::Arc,
};

use orchester_app::{
  root::Start,
  routes::{ProjectPage, Route, TaskView},
};
use orchester_core::{
  id::RunId,
  project::TaskKey,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::{RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use orchester_workspace::secrets::Vault;
use tokio::runtime::Runtime;

use super::{
  domain_support::Fixture,
  support::{Data, Window},
};

pub const PRIVATE_VALUE: &str = "private-known-metadata-value";
pub struct PrivateVault;
impl SecretVault for PrivateVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::Unsupported)
  }
  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Err(VaultError::Unsupported)
  }
  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    SecretValue::new(PRIVATE_VALUE.into()).map_err(|_| VaultError::Unsupported)
  }
}
pub struct Page {
  pub window: Window,
  pub fixture: Fixture,
  pub route: Route,
}
impl Page {
  pub fn new(fixture: Fixture) -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let data = Data {
      dir: tempfile::tempdir_in(scratch).expect("private app data"),
      runtime: Runtime::new().expect("runtime"),
    };
    let launch = data.launch();
    let project = launch
      .workspace()
      .expect("workspace")
      .open(fixture.root())
      .expect("register actual private repository");
    let db = ProjectDb::open(fixture.root()).expect("db");
    let run = db.run(fixture.run).expect("run").expect("exists");
    let key: TaskKey = db
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| task.id() == run.task())
      .expect("task")
      .key()
      .clone();
    let route = Route::Project(project.project.id(), ProjectPage::Task(key, TaskView::LocalIntegration));
    let mut start = launch.start();
    if let Start::Ready(ready) = &mut start {
      ready.services.vault = Vault::with(Arc::new(PrivateVault));
    }
    let window = Window::of_start(data, launch, start);
    window.navigation().go(route.clone());
    Self { window, fixture, route }
  }
  pub fn ready(&mut self, id: &str) {
    self.until(id, |window| window.tree.get_element_by_id(id).is_some());
  }
  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Window) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(&mut self.window) {
        return;
      }
      assert!(
        std::time::Instant::now() < deadline,
        "timed out waiting for {what}; operation errors: {:?}",
        self.window.texts("local-work-error")
      );
      std::thread::sleep(std::time::Duration::from_millis(5));
    }
  }
  pub fn click(&mut self, id: &str) {
    self.window.tree.get_element_by_id_mut(id).expect("control").click();
    self.window.pass();
  }
  pub fn set(&mut self, id: &str, value: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .expect("field")
      .as_text_input()
      .expect("editable")
      .set_value(value);
    self.window.pass();
  }
  pub fn type_into(&mut self, id: &str, text: &str) {
    let mut input = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .expect("field")
      .as_text_input()
      .expect("input");
    input.focus();
    input.select_all();
    for key in text.chars() {
      self
        .window
        .tree
        .key_down(key.to_string(), String::new(), false, false, false);
    }
    self.window.pass();
  }
  pub fn value(&mut self, id: &str) -> String {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .expect("field")
      .as_text_input()
      .expect("input")
      .value()
  }
  pub fn choose(&mut self, id: &str, label: &str) {
    let mut select = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .expect("select")
      .as_select()
      .expect("select");
    let index = select
      .labels()
      .iter()
      .position(|value| &**value == label)
      .expect("label");
    select.commit(index);
    self.window.pass();
  }
  pub fn has(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }
  pub fn run(&self) -> RunId {
    self.fixture.run
  }
  pub fn root(&self) -> &Path {
    self.fixture.root()
  }
}
pub fn contains(element: lurq::node::element::ElementRef<'_>, text: &str) -> bool {
  element.id().is_some_and(|id| id.contains(text))
    || element.text_content().is_some_and(|value| value.contains(text))
    || element.children().into_iter().any(|child| contains(child, text))
}
