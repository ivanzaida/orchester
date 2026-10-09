//! A window on a seeded project's Tasks list, tall enough that every row of the seed's 24 tasks is laid
//! out, and what the tests read off it.

use std::path::PathBuf;

use lurq::{app::events::MouseButton, core::ElementRect, node::element::ElementRef};
use orchester_app::routes::{ProjectPage, Route, TaskList};
use orchester_core::{id::ProjectId, time::Timestamp};

use crate::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window},
};

/// Room for the header, the query and all 24 rows with the count under them.
const TALL: (u32, u32) = (1440, 1700);

pub struct Seeded {
  pub window: Window,
  pub project: ProjectId,
  pub repo: PathBuf,
}

impl Seeded {
  /// `scenario` seeded into a new data folder, and its window, not yet on the Tasks list.
  pub fn new(scenario: &str) -> Self {
    Self::reading(scenario, false)
  }

  /// [`Seeded::new`], its project feeds' readings held from the start while `held`: the window lands on
  /// the project, whose feed would read its tasks before a test could hold them.
  pub fn reading(scenario: &str, held: bool) -> Self {
    let data = Data::new();
    let request = SeedRequest {
      data_dir: data.dir.path().to_path_buf(),
      repo: data.dir.path().join(format!("repo-{scenario}")),
      scenario: scenarios::named(scenario).expect("a known scenario"),
      fake_codex: None,
      now: Timestamp::now(),
    };
    let seeded = kontur::seed(&request).expect("the scenario seeds");
    let launch = data.launch();
    launch.services().expect("the session started").hold.set(held);
    let mut window = Window::of(data, launch);
    window.tree.resize(TALL.0, TALL.1);
    Self {
      window,
      project: seeded.project,
      repo: seeded.repo,
    }
  }

  /// `scenario` with its Tasks list shown at `list` once its tasks are read.
  pub fn tasks(scenario: &str, list: TaskList) -> Self {
    Self::tasks_in(scenario, list, TALL.1)
  }

  /// [`Seeded::tasks`] in a window `height` high, the design's 900 showing 13 rows.
  pub fn tasks_in(scenario: &str, list: TaskList, height: u32) -> Self {
    let mut seeded = Self::new(scenario);
    seeded.window.tree.resize(TALL.0, height);
    seeded.go(list);
    seeded.window.until("the tasks", |window| {
      window
        .texts("tasks-tab-all-label")
        .is_some_and(|label| !label.concat().ends_with('—'))
    });
    seeded
  }

  pub fn go(&mut self, list: TaskList) {
    self.window.navigation().go(self.route(list));
    self.window.pass();
  }

  pub fn route(&self, list: TaskList) -> Route {
    Route::Project(self.project, ProjectPage::Tasks(list))
  }

  /// The texts inside `id`, icon glyphs left out; panics when nothing has the id.
  pub fn texts(&mut self, id: &str) -> Vec<String> {
    self.window.texts(id).unwrap_or_else(|| panic!("{id} is shown"))
  }

  pub fn text(&mut self, id: &str) -> String {
    self.texts(id).join(" ")
  }

  pub fn shows(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }

  /// The keys of the rows shown, top to bottom.
  pub fn keys(&mut self) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(root) = self.window.tree.root() {
      collect_ids(root, "task-key-", &mut ids);
    }
    let mut rows: Vec<(f32, String)> = ids
      .into_iter()
      .filter_map(|id| {
        let key = id.strip_prefix("task-key-")?.to_owned();
        let bounds = self.window.tree.get_element_by_id_mut(&id)?.bounds()?;
        Some((bounds.y, key))
      })
      .collect();
    rows.sort_by(|left, right| left.0.total_cmp(&right.0));
    rows.into_iter().map(|(_, key)| key).collect()
  }

  /// The glyph of the icon `id`.
  pub fn glyph(&mut self, id: &str) -> char {
    self
      .window
      .tree
      .get_element_by_id(id)
      .and_then(|element| element.text_content()?.chars().next())
      .unwrap_or_else(|| panic!("{id} is an icon"))
  }

  /// Delivers what the window's futures finished until `done` holds.
  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
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

  /// Where `id` is laid out.
  pub fn bounds(&mut self, id: &str) -> ElementRect {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"))
  }

  /// Whether the row of `key` is laid out whole inside the table's room under its header.
  pub fn in_view(&mut self, key: &str) -> bool {
    let id = format!("task-row-{key}");
    let Some(row) = self.window.tree.get_element_by_id_mut(&id).and_then(|row| row.bounds()) else {
      return false;
    };
    let (table, header) = (self.bounds("tasks-table"), self.bounds("tasks-table-header"));
    row.y >= header.y + header.height - 0.5 && row.y + row.height <= table.y + table.height + 0.5
  }

  /// A press with the pointer at the middle of `id`.
  pub fn click(&mut self, id: &str) {
    let bounds = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    let tree = &mut self.window.tree;
    tree.mouse_move(x, y);
    tree.mouse_down(x, y, MouseButton::Left);
    tree.mouse_up(x, y, MouseButton::Left);
    self.window.pass();
  }
}

/// The ids under `element` that start with `prefix`.
fn collect_ids(element: ElementRef<'_>, prefix: &str, into: &mut Vec<String>) {
  if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
    into.push(id.to_owned());
  }
  for child in element.children() {
    collect_ids(child, prefix, into);
  }
}
