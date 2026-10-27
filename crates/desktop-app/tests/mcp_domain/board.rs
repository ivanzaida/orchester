//! The Tasks board (ORC-105) with the window's MCP server: a bulk Mark as done an MCP client presses is recorded as
//! automation's, since who asks is read in the press itself; a complete draft moved to Todo is queued there; and with
//! the real runner serving the project and an agent slot free, queuing a draft on Todo, alone from its card's Move to
//! or several from the selection bar, asks first that it starts now, and Keep as draft queues nothing.
//! `cargo test -p orchester-app --test mcp_domain domain_board_`.

use std::path::Path;

use lurq::app::events::MouseButton;
use orchester_app::routes::{ProjectPage, Route, TaskList, TaskListTab};
use orchester_core::{
  id::ProjectId,
  store::TaskStore,
  task::{
    Task, TaskStatus,
    organization::{ChangeActor, WorkflowStatus},
  },
};
use orchester_store::ProjectDb;
use serde_json::json;

use super::{Served, runner_programs, tools_window::text};

/// Room for every card of the seed's columns.
const TALL: (u32, u32) = (1440, 1700);

/// The task of `repo` with `key`, as stored.
fn stored(repo: &Path, key: &str) -> Task {
  ProjectDb::open(repo)
    .expect("the project's data")
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.key().to_string() == key)
    .unwrap_or_else(|| panic!("task {key}"))
}

/// Who changed the organization of task `key` last.
fn last_actor(repo: &Path, key: &str) -> ChangeActor {
  let task = stored(repo, key);
  ProjectDb::open(repo)
    .expect("the project's data")
    .task_organization_history(task.id())
    .expect("history")
    .last()
    .expect("a change")
    .actor
}

impl Served {
  /// `project`'s Tasks at All, switched to the board by the person.
  fn board(&mut self, project: ProjectId) {
    self.tree.resize(TALL.0, TALL.1);
    let navigation = self.link.navigation().expect("the window attached its navigation");
    navigation.go(Route::Project(
      project,
      ProjectPage::Tasks(TaskList::new(TaskListTab::All)),
    ));
    self.until("the layout switch", |served| served.laid_out("tasks-layout-board"));
    self.click_by_person("tasks-layout-board");
    self.until("the board", |served| served.laid_out("tasks-board-columns"));
  }

  fn laid_out(&mut self, id: &str) -> bool {
    self
      .tree
      .get_element_by_id_mut(id)
      .is_some_and(|element| element.bounds().is_some())
  }

  /// The middle of `id`, once it is laid out.
  fn middle_laid_out(&mut self, id: &str) -> (f32, f32) {
    self.until(id, |served| served.laid_out(id));
    let bounds = self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} laid out"));
    (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0)
  }

  /// The person's Shift+click on `id`.
  fn shift_click(&mut self, id: &str) {
    let (x, y) = self.middle_laid_out(id);
    self.tree.mouse_move_with_meta(x, y, true, false, false, false);
    self
      .tree
      .mouse_down_with_meta(x, y, MouseButton::Left, true, false, false, false);
    self
      .tree
      .mouse_up_with_meta(x, y, MouseButton::Left, true, false, false, false);
    self.pass();
  }

  /// The person's press on `id`, once it is laid out.
  fn press_laid_out(&mut self, id: &str) {
    self.until(id, |served| served.laid_out(id));
    self.click_by_person(id);
  }

  /// Complete Quick fix drafts titled `titles`, made through `create_task`: their keys.
  fn drafts(&mut self, titles: &[&str]) -> Vec<String> {
    titles
      .iter()
      .map(|title| {
        let created = self.ok(
          "create_task",
          &json!({
            "project": "Kontur", "title": title, "brief": "Do it.", "criteria": ["It is done."],
            "pipeline": "Quick fix",
          }),
        );
        created["task"]["key"].as_str().expect("a key").to_owned()
      })
      .collect()
  }

  /// The person moves the card of `key` to the status column `column` from its menu's Move to.
  fn move_from_menu(&mut self, key: &str, column: &str) {
    let (x, y) = self.middle_laid_out(&format!("task-card-{key}"));
    self.tree.mouse_move(x, y);
    self.pass();
    self.press_laid_out(&format!("task-more-{key}"));
    self.press_laid_out(&format!("task-menu-{key}-move"));
    self.press_laid_out(&format!("task-menu-{key}-move-{column}"));
  }
}

#[test]
fn domain_board_bulk_mark_as_done_pressed_by_a_client_is_automation_s() {
  let (folder, seeded) = Served::seed("tasks", None);
  let runner = folder.path().join("no-runner").join("orchester-runner");
  let mut served = Served::start_with(folder, &seeded.app_database, runner, true);
  served.board(seeded.project);
  // KON-15 is a draft without a run: closing it stops nothing.
  served.shift_click("task-card-KON-15");
  served.until("the selection bar", |served| served.laid_out("tasks-board-bulk-done"));
  served
    .click_by_client("tasks-board-bulk-done")
    .expect("a client may mark tasks as done");
  served.until("KON-15 Done", |_| {
    stored(&seeded.repo, "KON-15").organization().status() == WorkflowStatus::Done
  });
  assert_eq!(last_actor(&seeded.repo, "KON-15"), ChangeActor::Orchester);
}

#[test]
fn domain_board_a_complete_draft_moved_to_todo_is_queued() {
  // The Codex connection's program is any file the seed records as checked; no runner serves the project, so queuing
  // starts nothing now and asks nothing.
  let codex = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
  let (folder, seeded) = Served::seed("empty", Some(codex));
  let runner = folder.path().join("no-runner").join("orchester-runner");
  let mut served = Served::start_with(folder, &seeded.app_database, runner, true);
  let key = served.drafts(&["Write the board's docs"]).remove(0);
  served.board(seeded.project);
  assert_eq!(stored(&seeded.repo, &key).status(), TaskStatus::Draft);
  served.move_from_menu(&key, "todo");
  // Queuing writes the run's queue first and Todo after it: wait for both.
  served.until("the draft queued in Todo", |_| {
    let task = stored(&seeded.repo, &key);
    task.status() == TaskStatus::Queued && task.organization().status() == WorkflowStatus::Todo
  });
  assert!(!served.shows("tasks-board-ask"), "nothing starts now: nothing asked");
  assert_eq!(last_actor(&seeded.repo, &key), ChangeActor::Person);
}

#[test]
fn domain_board_queuing_drafts_that_start_now_asks_first_alone_and_in_bulk() {
  let programs = runner_programs::programs();
  let (folder, seeded) = Served::seed("empty", Some(programs.fake_codex.clone()));
  let mut served = Served::start_with(folder, &seeded.app_database, programs.runner.clone(), true);
  let keys = served.drafts(&["Write the board's docs", "Explain the board's keys"]);
  served.board(seeded.project);
  served.until("the runner online with a free agent slot", |served| {
    served
      .services()
      .runners
      .status()
      .work()
      .is_some_and(|work| usize::try_from(work.agent_slots.get()).is_ok_and(|slots| slots > work.agents_working()))
  });

  // Alone, from its card's Move to.
  let first = &keys[0];
  served.move_from_menu(first, "todo");
  served.until("the question", |served| served.laid_out("tasks-board-ask-keep"));
  let asked = text(&mut served, "tasks-board-ask-title");
  assert!(asked.starts_with(&format!("Queue {first}? It starts now: ")), "{asked}");
  served.click_by_person("tasks-board-ask-keep");
  assert!(!served.shows("tasks-board-ask"));
  assert_eq!(stored(&seeded.repo, first).status(), TaskStatus::Draft, "kept as draft");

  // Both, from the selection bar's Move to.
  for key in &keys {
    served.shift_click(&format!("task-card-{key}"));
  }
  served.press_laid_out("tasks-board-bulk-move");
  served.press_laid_out("tasks-board-bulk-move-todo");
  served.until("the bulk question", |served| served.laid_out("tasks-board-ask-keep"));
  let asked = text(&mut served, "tasks-board-ask-title");
  assert!(
    asked.starts_with(&format!("Queue {} and {}? They start now: ", keys[0], keys[1])),
    "{asked}"
  );
  served.click_by_person("tasks-board-ask-keep");
  for key in &keys {
    let task = stored(&seeded.repo, key);
    assert_eq!(task.status(), TaskStatus::Draft, "{key} kept as draft");
    assert_eq!(task.organization().status(), WorkflowStatus::Backlog);
  }
}
