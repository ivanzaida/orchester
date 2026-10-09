//! A window on a seeded Kontur (`examples/seed`) whose runner program does not exist, so a run never
//! starts: the form is driven by element id, as a person fills it in, and what it stored is read back from
//! the project's database.

use std::{fs, path::Path};

use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{project::TaskKey, store::TaskStore, task::Task};
use orchester_store::ProjectDb;

use crate::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window},
};

pub struct Form {
  pub window: Window,
  pub project: orchester_core::id::ProjectId,
  pub repo: std::path::PathBuf,
}

impl Form {
  /// The window on `scenario` (`live`, `tasks`), seeded in the window's own data folder.
  pub fn seeded(scenario: &str) -> Self {
    let data = Data::new();
    let fake_codex = data.dir.path().join("fake-codex");
    fs::write(&fake_codex, b"").expect("a stand-in for the fake Codex");
    let seeded = kontur::seed(&SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: data.dir.path().join(format!("repo-{scenario}")),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: Some(fake_codex),
      now: orchester_core::time::Timestamp::now(),
    })
    .expect("the scenario seeds");
    let launch = data.launch();
    Self {
      window: Window::of(data, launch),
      project: seeded.project,
      repo: seeded.repo,
    }
  }

  /// The window on Kontur without a pipeline (`support::Window::open`).
  pub fn bare() -> Self {
    let window = Window::open();
    let repo = window.data.dir.path().join("repo-kontur");
    Self {
      window,
      project: crate::support::project_id(),
      repo,
    }
  }

  pub fn go(&mut self, page: ProjectPage) {
    self.window.navigation().go(Route::Project(self.project, page));
    self.window.pass();
  }

  /// New task, once its fields are filled.
  pub fn new_task(&mut self) {
    self.go(ProjectPage::NewTask);
    self
      .window
      .until("New task's form", |window| window.texts("task-form-title").is_some());
  }

  /// Edit task on `key`, once its fields are filled.
  pub fn edit(&mut self, key: &str) {
    self.go(ProjectPage::EditTask(key.parse().expect("a task key")));
    self
      .window
      .until("Edit task's form", |window| window.texts("task-form-title").is_some());
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

  pub fn value(&mut self, id: &str) -> String {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} is a text input"))
      .value()
  }

  /// Chooses the pipeline labelled `label` in the select.
  pub fn choose_pipeline(&mut self, label: &str) {
    let mut select = self
      .window
      .tree
      .get_element_by_id_mut("task-form-pipeline-input")
      .and_then(|element| element.as_select())
      .expect("the pipeline select");
    let index = select
      .labels()
      .iter()
      .position(|option| &**option == label)
      .unwrap_or_else(|| panic!("an option {label}"));
    select.commit(index);
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

  /// Every reason and problem under the actions.
  pub fn reasons(&mut self) -> Vec<String> {
    self.window.texts("task-form-reasons").unwrap_or_default()
  }

  pub fn status(&mut self) -> String {
    self.text("task-form-status-label")
  }

  /// Fills a task that can run on `pipeline`.
  pub fn fill(&mut self, title: &str, pipeline: &str) {
    self.set("task-form-title-input", title);
    self.set("task-form-brief-input", "Record every delivery attempt.");
    self.click("task-form-add-criterion");
    self.set("task-form-criterion-1-input", "Every attempt is stored.");
    self.choose_pipeline(pipeline);
    self.set("task-form-budget-input", "1,400,000");
    self.set("task-form-time-limit-input", "30");
  }

  pub fn db(&self) -> ProjectDb {
    db(&self.repo)
  }

  pub fn task(&self, key: &str) -> Option<Task> {
    let key: TaskKey = key.parse().expect("a task key");
    self
      .db()
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| *task.key() == key)
  }
}

fn db(repo: &Path) -> ProjectDb {
  ProjectDb::open(repo).expect("the project database opens")
}
