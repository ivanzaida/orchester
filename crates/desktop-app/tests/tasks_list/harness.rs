//! A window on a seeded project's Tasks list, tall enough that every row of the seed's 24 tasks is laid
//! out, and what the tests read off it.

use std::path::PathBuf;

use lurq::{app::events::MouseButton, core::ElementRect, node::element::ElementRef};
use orchester_app::{
  project::TasksState,
  routes::{ProjectPage, Route, TaskList},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::tasks::data::ProjectTasks;

use crate::{
  kontur::{self, SeedRequest, scenarios},
  support::{Data, Window},
};

/// Room for the header, the views and all 24 rows of All with its six groups' headers.
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

  /// [`Seeded::tasks`] in a window `height` high, the design's 900. The list may lay its rows out over more than one
  /// pass: it waits until every row it holds is laid out.
  pub fn tasks_in(scenario: &str, list: TaskList, height: u32) -> Self {
    let mut seeded = Self::new(scenario);
    seeded.window.tree.resize(TALL.0, height);
    seeded.go(list);
    seeded.window.until("the tasks", |window| {
      window
        .texts("tasks-view-all-count")
        .is_some_and(|count| !count.is_empty() && count.concat() != "—")
    });
    seeded.until("the rows laid out", Self::rows_laid_out);
    seeded
  }

  /// The view `view`'s label and count, as `Active 18`.
  pub fn view(&mut self, view: &str) -> String {
    self.text(&format!("tasks-view-{view}"))
  }

  /// The keys of the rows under each group's header, top to bottom, by the group's name (`needs_you`, `running`…).
  pub fn groups(&mut self) -> Vec<(String, Vec<String>)> {
    let mut ids = Vec::new();
    if let Some(root) = self.window.tree.root() {
      collect_ids(root, "tasks-group-", &mut ids);
      collect_ids(root, "task-key-", &mut ids);
    }
    let mut placed: Vec<(f32, String)> = ids
      .into_iter()
      .filter(|id| id.starts_with("task-key-") || !id.trim_start_matches("tasks-group-").contains('-'))
      .filter_map(|id| Some((self.window.tree.get_element_by_id_mut(&id)?.bounds()?.y, id)))
      .collect();
    placed.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for (_, id) in placed {
      if let Some(group) = id.strip_prefix("tasks-group-") {
        groups.push((group.to_owned(), Vec::new()));
      } else if let Some(last) = groups.last_mut() {
        last.1.push(id.trim_start_matches("task-key-").to_owned());
      }
    }
    groups
  }

  /// The pointer moves onto the middle of `id`.
  pub fn hover(&mut self, id: &str) {
    let bounds = self.bounds(id);
    self
      .window
      .tree
      .mouse_move(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.window.pass();
  }

  /// The project's tasks as the list shows them: no runner serves the seeded project, so as they stand without one.
  pub fn shown_tasks(&self) -> ProjectTasks {
    match self.window.projects().feed(self.project).tasks.get() {
      TasksState::Ready(tasks) => tasks.without_runner(),
      TasksState::Loading | TasksState::Failed(_) => panic!("the tasks are read"),
    }
  }

  /// Whether every row the list holds is laid out.
  pub fn rows_laid_out(&mut self) -> bool {
    let mut ids = Vec::new();
    if let Some(root) = self.window.tree.root() {
      collect_ids(root, "task-key-", &mut ids);
    }
    ids.iter().all(|id| self.laid_out(id))
  }

  /// Whether `id` is shown and laid out: an element can be there a pass before it has its place.
  pub fn laid_out(&mut self, id: &str) -> bool {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .is_some_and(|element| element.bounds().is_some())
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

  /// Whether the row of `key` is laid out whole inside the list's room.
  pub fn in_view(&mut self, key: &str) -> bool {
    let id = format!("task-row-{key}");
    let Some(row) = self.window.tree.get_element_by_id_mut(&id).and_then(|row| row.bounds()) else {
      return false;
    };
    let list = self.bounds("tasks-list");
    row.y >= list.y - 0.5 && row.y + row.height <= list.y + list.height + 0.5
  }

  /// Scrolls the rows far down with the wheel.
  pub fn scroll_down(&mut self) {
    let list = self.bounds("tasks-list");
    let (x, y) = (list.x + list.width / 2.0, list.y + list.height / 2.0);
    self
      .window
      .tree
      .scroll(x, y, 0.0, -2000.0, lurq::app::events::ScrollPhase::Scroll);
    self.window.pass();
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

  /// A Shift+click at the middle of `id`.
  pub fn shift_click(&mut self, id: &str) {
    let bounds = self.bounds(id);
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    let tree = &mut self.window.tree;
    tree.mouse_move_with_meta(x, y, true, false, false, false);
    tree.mouse_down_with_meta(x, y, MouseButton::Left, true, false, false, false);
    tree.mouse_up_with_meta(x, y, MouseButton::Left, true, false, false, false);
    self.window.pass();
  }

  /// `scenario`'s Tasks at `list`, switched to the board once its tasks are read.
  pub fn board(scenario: &str, list: TaskList) -> Self {
    let mut seeded = Self::tasks(scenario, list);
    seeded.until("the layout switch", |seeded| seeded.laid_out("tasks-layout-board"));
    seeded.click("tasks-layout-board");
    seeded.until("the board", |seeded| seeded.laid_out("tasks-board-columns"));
    seeded
  }

  /// Lifts the card of `key` with the pointer and drops it on the column `column`, a little under its head, moving in
  /// steps as a hand does.
  pub fn drag(&mut self, key: &str, column: &str) {
    let (_, (x1, y1)) = self.hold(key, column);
    self.window.tree.mouse_up(x1, y1, MouseButton::Left);
    self.window.pass();
  }

  /// Lifts the card of `key` and holds it over the column `column`, the button still down; answers where the card
  /// was taken from and where the pointer is.
  pub fn hold(&mut self, key: &str, column: &str) -> ((f32, f32), (f32, f32)) {
    self.hold_over(key, &format!("tasks-board-column-{column}"))
  }

  /// Lifts the card of `key` and holds it over the element `id`, a little under its top, the button still down;
  /// answers where the card was taken from and where the pointer is.
  pub fn hold_over(&mut self, key: &str, id: &str) -> ((f32, f32), (f32, f32)) {
    let card = format!("task-card-{key}");
    self.until(&card, |seeded| seeded.laid_out(&card));
    let (from, to) = (self.bounds(&card), self.bounds(id));
    let (x0, y0) = (from.x + from.width / 2.0, from.y + from.height / 2.0);
    let (x1, y1) = (to.x + to.width / 2.0, to.y + DROP_DEPTH.min(to.height / 2.0));
    self.window.tree.mouse_move(x0, y0);
    self.window.tree.mouse_down(x0, y0, MouseButton::Left);
    self.window.pass();
    for step in 1..=DRAG_STEPS {
      let part = f32::from(step) / f32::from(DRAG_STEPS);
      self
        .window
        .tree
        .mouse_move(x0 + (x1 - x0) * part, y0 + (y1 - y0) * part);
      self.window.pass();
    }
    ((x0, y0), (x1, y1))
  }

  /// The ids under the window's root that start with `prefix`.
  pub fn ids(&mut self, prefix: &str) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(root) = self.window.tree.root() {
      collect_ids(root, prefix, &mut ids);
    }
    ids
  }
}

/// How far under a column's top a drag lets go, and in how many moves it gets there.
const DROP_DEPTH: f32 = 80.0;
const DRAG_STEPS: u16 = 8;

/// The ids under `element` that start with `prefix`.
fn collect_ids(element: ElementRef<'_>, prefix: &str, into: &mut Vec<String>) {
  if let Some(id) = element.id().filter(|id| id.starts_with(prefix)) {
    into.push(id.to_owned());
  }
  for child in element.children() {
    collect_ids(child, prefix, into);
  }
}
