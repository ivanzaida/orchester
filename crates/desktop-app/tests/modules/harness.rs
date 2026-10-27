//! A window on a seeded Kontur (`examples/seed`) whose project has modules made before the window opened, driven by
//! element id as a person uses it, with what it stored read back from the project's database.

use std::path::PathBuf;

use chrono::{Days, NaiveDate};
use orchester_app::routes::{ProjectPage, Route};
use orchester_core::{
  id::{ModuleId, ProjectId},
  module::{Module, ModuleDetails, ModuleState},
  name::Name,
  project::TaskKey,
  store::TaskStore,
  task::{
    Task,
    organization::{ChangeActor, WorkflowStatus},
  },
  time::Timestamp,
  usage::local_date,
};
use orchester_store::ProjectDb;

use crate::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window},
};

/// Room for a module page's header, its burndown card and its tasks.
const TALL: (u32, u32) = (1440, 1700);

pub struct Seeded {
  pub window: Window,
  pub project: ProjectId,
  pub repo: PathBuf,
}

impl Seeded {
  /// `scenario` seeded into a new data folder; `prepare` changes its project before the window opens on it.
  pub fn new(scenario: &str, prepare: impl FnOnce(&mut ProjectDb)) -> Self {
    let data = Data::new();
    let fake_codex = data.dir.path().join("fake-codex");
    std::fs::write(&fake_codex, b"").expect("a stand-in for the fake Codex");
    let seeded = kontur::seed(&SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: data.dir.path().join(format!("repo-{scenario}")),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: Some(fake_codex),
      now: Timestamp::now(),
    })
    .expect("the scenario seeds");
    prepare(&mut ProjectDb::open(&seeded.repo).expect("the project database opens"));
    let launch = data.launch();
    let mut window = Window::of(data, launch);
    window.tree.resize(TALL.0, TALL.1);
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

  /// Goes to `page` and waits until `id` is shown.
  pub fn open(&mut self, page: ProjectPage, id: &str) {
    self.go(page);
    self.until(id, |seeded| seeded.shows(id));
  }

  pub fn route(&self) -> Route {
    self.window.navigation().route_now()
  }

  pub fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// The texts inside `id` on one line, icon glyphs left out; panics when nothing has the id.
  pub fn text(&mut self, id: &str) -> String {
    self.window.text(id)
  }

  pub fn set(&mut self, id: &str, value: &str) {
    self.window.set(id, value);
  }

  /// Clicks `id` as its handler takes a click.
  pub fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} is shown"))
      .click();
    self.window.pass();
  }

  /// The labels of the select `id`'s options.
  pub fn options(&mut self, id: &str) -> Vec<String> {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .unwrap_or_else(|| panic!("{id} is a select"))
      .labels()
      .iter()
      .map(|label| label.to_string())
      .collect()
  }

  /// Chooses the option of the select `id` labelled `label`.
  pub fn choose(&mut self, id: &str, label: &str) {
    let mut select = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .unwrap_or_else(|| panic!("{id} is a select"));
    let index = select
      .labels()
      .iter()
      .position(|option| &**option == label)
      .unwrap_or_else(|| panic!("{id} offers {label}: {:?}", select.labels()));
    select.commit(index);
    self.window.pass();
  }

  /// Delivers what the window's futures finished until `done` holds. The deadline leaves room for the binary's
  /// tests seeding their repositories side by side, which slows every reading of a project's tasks.
  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(self) {
        return;
      }
      assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
      std::thread::sleep(std::time::Duration::from_millis(5));
    }
  }

  /// Has the window read the project's tasks again, as a save does.
  pub fn read_again(&mut self) {
    self.window.projects().feed(self.project).saved();
    self.window.pass();
  }

  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("the project database opens")
  }

  pub fn task(&self, key: &str) -> Task {
    let key: TaskKey = key.parse().expect("a task key");
    self
      .db()
      .tasks()
      .expect("tasks")
      .into_iter()
      .find(|task| *task.key() == key)
      .unwrap_or_else(|| panic!("{key} is stored"))
  }

  pub fn module_named(&self, name: &str) -> Option<Module> {
    self
      .db()
      .modules()
      .expect("modules")
      .into_iter()
      .find(|module| module.name().as_str() == name)
  }

  /// What follows `prefix` in the ids of the window's elements that start with it, sorted.
  pub fn ids(&mut self, prefix: &str) -> Vec<String> {
    fn collect(element: lurq::node::element::ElementRef<'_>, prefix: &str, into: &mut Vec<String>) {
      if let Some(rest) = element.id().and_then(|id| id.strip_prefix(prefix)) {
        into.push(rest.to_owned());
      }
      for child in element.children() {
        collect(child, prefix, into);
      }
    }
    let mut ids = Vec::new();
    if let Some(root) = self.window.tree.root() {
      collect(root, prefix, &mut ids);
    }
    ids.sort();
    ids
  }

  /// A key pressed with nothing held.
  pub fn key(&mut self, key: &str) {
    self.window.tree.key_down(key.into(), key.into(), false, false, false);
    self.window.pass();
  }

  /// Where `id` is laid out.
  pub fn bounds(&mut self, id: &str) -> lurq::core::ElementRect {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }
}

/// `details` with `description` and in `state`.
pub fn with(details: &ModuleDetails, description: &str, state: ModuleState) -> ModuleDetails {
  ModuleDetails::new(
    details.name().clone(),
    description,
    (details.start(), details.target()),
    state,
  )
  .expect("valid details")
}

/// Today, as the module pages count it.
pub fn today() -> NaiveDate {
  local_date(Timestamp::now())
}

/// Module details named `name`, from `before` days ago to `after` days ahead.
pub fn details(name: &str, (before, after): (u64, u64)) -> ModuleDetails {
  let start = today().checked_sub_days(Days::new(before)).expect("a start");
  let target = today().checked_add_days(Days::new(after)).expect("a target");
  ModuleDetails::new(
    Name::new(name).expect("a name"),
    "",
    (Some(start), Some(target)),
    ModuleState::Open,
  )
  .expect("valid details")
}

/// Makes a module with `details` and puts in it, for each of `picks`, as many tasks with that workflow status; the
/// keys put in.
pub fn module_with(
  db: &mut ProjectDb,
  details: ModuleDetails,
  picks: &[(WorkflowStatus, usize)],
) -> (ModuleId, Vec<String>) {
  let module = db.create_module(details, Timestamp::now()).expect("a module");
  let stored = db.tasks().expect("tasks");
  let tasks: Vec<&Task> = picks
    .iter()
    .flat_map(|(status, count)| {
      let picked: Vec<&Task> = stored
        .iter()
        .filter(|task| task.organization().status() == *status)
        .take(*count)
        .collect();
      assert_eq!(picked.len(), *count, "the seed has {count} tasks {status:?}");
      picked
    })
    .collect();
  for task in &tasks {
    db.set_task_module(task.id(), Some(module.id()), ChangeActor::Person, Timestamp::now())
      .expect("in the module");
  }
  (module.id(), tasks.iter().map(|task| task.key().to_string()).collect())
}
