//! The window on a seeded Kontur (`examples/seed`, the `memory` scenario unless a test names another), with a
//! keychain in memory, and what the tests read of the project database.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use orchester_app::routes::{MemoryTab, ProjectPage, Route, TaskView};
use orchester_core::{
  id::{MemoryId, ProjectId, SecretId},
  memory::{Memory, MemoryFilter, MemoryStore, ScopeFilter},
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  store::{SecretStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::secrets::Vault;
use parking_lot::Mutex;
use tempfile::TempDir;
use tokio::runtime::Runtime;

use super::{
  kontur,
  support::{Data, Window},
};

/// A keychain in memory: tests never touch the keychain of the user running them.
#[derive(Default)]
struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

pub struct Page {
  pub window: Window,
  pub project: ProjectId,
  pub repo: PathBuf,
  vault: Arc<MemoryVault>,
}

impl Page {
  /// The window on the seeded `scenario`.
  pub fn seeded(scenario: &str) -> Self {
    let dir = TempDir::new().expect("a temporary folder");
    let request = kontur::SeedRequest {
      data_dir: dir.path().to_path_buf(),
      repo: dir.path().join(format!("repo-{scenario}")),
      scenario: kontur::scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let data = Data::at(dir, Runtime::new().expect("tokio runtime"));
    let launch = data.launch();
    let mut start = launch.start();
    let vault = Arc::new(MemoryVault::default());
    if let orchester_app::root::Start::Ready(ready) = &mut start {
      ready.services.vault = Vault::with(vault.clone());
    }
    let mut window = Window::of_start(data, launch, start);
    // Tall enough that every record of the seed is laid out within the window, so a test can point at it.
    window.tree.resize(1440, 2400);
    Self {
      window,
      project: seeded.project,
      repo: seeded.repo,
      vault,
    }
  }

  /// Changes the project database with `sql` the way an edit outside Orchester would, `id` its `?1`; how many rows
  /// it changed.
  pub fn edit_outside(&self, sql: &str, id: Option<MemoryId>) -> usize {
    let raw = rusqlite::Connection::open(self.repo.join(".orchester").join("orchester.db")).expect("the database");
    match id {
      Some(id) => raw.execute(sql, [id.to_string()]),
      None => raw.execute(sql, []),
    }
    .expect("the edit")
  }

  /// A secret of the project named `name`, its `value` in the keychain the window reads.
  pub fn secret(&self, name: &str, value: &str) {
    let mut db = self.db();
    let secret = Secret::new(
      SecretId::generate(),
      NewSecret {
        name: SecretName::new(name).expect("a secret name"),
        kind: SecretKind::AccessToken,
        environment: Environment::Staging,
      },
      Timestamp::now(),
    );
    db.create_secret(&secret).expect("the secret");
    self
      .vault
      .store(
        &secret.vault_key(db.project().id()),
        &SecretValue::new(value.to_owned()).expect("a value"),
      )
      .expect("stored");
  }

  /// Opens the memory of task `key`, the view its Summary's properties link to, and waits until its records are read.
  pub fn task_memory(&mut self, key: &str) {
    let page = ProjectPage::Task(key.parse().expect("a task key"), TaskView::Memory);
    self.window.navigation().go(Route::Project(self.project, page));
    self.window.until(&format!("the memory of {key}"), |window| {
      window.texts("memory-list").is_some() || window.texts("memory-load-error").is_some()
    });
  }

  /// Opens the Summary of task `key` and waits until its properties end in the link to its memory.
  pub fn task_summary(&mut self, key: &str) {
    let page = ProjectPage::Task(key.parse().expect("a task key"), TaskView::Summary);
    self.window.navigation().go(Route::Project(self.project, page));
    self.window.until(&format!("the summary of {key}"), |window| {
      window.texts("task-memory-link-label").is_some()
    });
  }

  /// Opens the project's Memory page on the tab it opens on and waits until it is read.
  pub fn project_memory(&mut self) {
    self.open_memory(None);
  }

  /// Opens the project's Memory page on `tab` and waits until it is read.
  pub fn memory_tab(&mut self, tab: MemoryTab) {
    self.open_memory(Some(tab));
  }

  fn open_memory(&mut self, tab: Option<MemoryTab>) {
    self
      .window
      .navigation()
      .go(Route::Project(self.project, ProjectPage::Memory(tab)));
    self.window.until("the project's memory", |window| {
      window.texts("memory-body").is_some() || window.texts("memory-load-error").is_some()
    });
  }

  /// The page's tabs in their order from the left, by name (`waiting`, `active`, …), and the one it shows.
  pub fn tabs(&mut self) -> (Vec<&'static str>, &'static str) {
    let mut placed: Vec<(f32, &'static str)> = MemoryTab::ALL
      .into_iter()
      .filter_map(|tab| Some((self.bounds_of(&format!("memory-tab-{}", tab.name()))?.x, tab.name())))
      .collect();
    placed.sort_by(|(left, _), (right, _)| left.total_cmp(right));
    let shown = MemoryTab::ALL
      .into_iter()
      .find(|tab| {
        let body = match tab {
          MemoryTab::Settings => "memory-settings".to_owned(),
          tab => format!("memory-tab-body-{}", tab.name()),
        };
        self.has(&body)
      })
      .expect("a tab's body is shown");
    (placed.into_iter().map(|(_, name)| name).collect(), shown.name())
  }

  fn bounds_of(&mut self, id: &str) -> Option<lurq::core::ElementRect> {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
  }

  /// Moves the keyboard's focus to the element `id`, as pressing another field does, so the field it leaves saves.
  pub fn focus(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .focus();
    self.window.pass();
  }

  /// What the text input `id` holds.
  pub fn value(&mut self, id: &str) -> String {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .value()
  }

  /// The ids of the elements under `id` that end past `right`, the window's right edge.
  pub fn past_right_edge(&mut self, id: &str, right: f32) -> Vec<String> {
    fn ids(element: lurq::node::element::ElementRef<'_>, into: &mut Vec<String>) {
      for child in element.children() {
        if let Some(id) = child.id() {
          into.push(id.to_owned());
        }
        ids(child, into);
      }
    }
    let mut inner = Vec::new();
    ids(self.window.tree.get_element_by_id(id).expect("shown"), &mut inner);
    assert!(inner.len() > 20, "{id} holds identified elements");
    inner
      .into_iter()
      .filter(|child| {
        self
          .bounds_of(child)
          .is_some_and(|rect| rect.width > 0.0 && rect.x + rect.width > right + 0.5)
      })
      .collect()
  }

  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("the project database")
  }

  /// Every record of the project, of every scope.
  pub fn records(&self) -> Vec<Memory> {
    self
      .db()
      .memories(&MemoryFilter {
        scope: ScopeFilter::All,
        status: None,
        pinned_only: false,
      })
      .expect("records")
  }

  /// The record that says `text` now.
  pub fn record(&self, text: &str) -> Memory {
    self
      .records()
      .into_iter()
      .find(|record| record.content().text().starts_with(text))
      .unwrap_or_else(|| panic!("a record saying {text}"))
  }

  pub fn stored(&self, id: MemoryId) -> Option<Memory> {
    self.db().memory(id).expect("the record reads")
  }

  /// Where the element `id` is laid out.
  pub fn bounds(&mut self, id: &str) -> lurq::core::ElementRect {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }

  /// The id of task `key`.
  pub fn task(&self, key: &str) -> orchester_core::id::TaskId {
    self
      .db()
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| task.key().to_string() == key)
      .expect("the task")
      .id()
  }

  pub fn has(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  pub fn text(&mut self, id: &str) -> String {
    self.window.text(id)
  }

  /// Moves the pointer over the element `id`, so a row shows its actions.
  pub fn hover(&mut self, id: &str) {
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    self
      .window
      .tree
      .mouse_move(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.window.pass();
  }

  /// Presses `id` and waits until `done` holds.
  pub fn press_until(&mut self, id: &str, what: &str, done: impl FnMut(&mut Window) -> bool) {
    self.window.press(id);
    self.window.until(what, done);
  }

  /// Delivers what the window's futures finished until `done` holds.
  pub fn until(&mut self, what: &str, done: impl FnMut(&mut Window) -> bool) {
    self.window.until(what, done);
  }
}
