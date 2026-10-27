//! A module's page (`02 · Module · Summary`, `03 · Module · Burndown`, `07`): its tabs Summary and Burndown, each its
//! own route, which back and forward return to; the header at the task page's own places; the Summary's tasks by
//! workflow status with Done folded, its burndown line and its properties saved in place; the burndown as a chart or a
//! table; *More actions* closing and reopening the module; *New task in module*; a module that is gone, and a page
//! that cannot be read again shown as such.

use chrono::Days;
use orchester_app::routes::{ModuleTab, ProjectPage, Route, TaskView};
use orchester_core::{module::ModuleState, store::TaskStore, task::organization::WorkflowStatus};

use super::harness::{Seeded, details, module_with, today};

fn rows(seeded: &mut Seeded, prefix: &str) -> usize {
  seeded
    .ids(prefix)
    .iter()
    .filter(|rest| rest.parse::<usize>().is_ok())
    .count()
}

#[test]
fn modules_page_has_summary_and_burndown_tabs_of_their_own() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    let picks = [(WorkflowStatus::Done, 1), (WorkflowStatus::Todo, 2)];
    picked = Some(module_with(db, details("Checkout", (3, 4)), &picks));
  });
  let (module, keys) = picked.expect("the module");
  let summary = Route::Project(seeded.project, ProjectPage::Module(module, ModuleTab::Summary));
  let burndown = Route::Project(seeded.project, ProjectPage::Module(module, ModuleTab::Burndown));
  seeded.open(ProjectPage::Module(module, ModuleTab::Summary), "module-summary");
  assert_eq!(seeded.text("module-title"), "Checkout");
  assert_eq!(seeded.text("module-state"), "Open");
  let facts = seeded.text("module-facts");
  assert!(
    facts.contains("1 of 3 tasks done") && facts.contains("Not enough history"),
    "{facts}"
  );
  assert_eq!(seeded.text("module-tasks-head"), "Tasks 3");
  let (done, todo) = (&keys[0], &keys[1]);
  assert!(seeded.shows(&format!("module-task-{todo}")), "Todo is open");
  assert!(!seeded.shows(&format!("module-task-{done}")), "Done is folded");
  seeded.click("module-group-done");
  assert!(seeded.shows(&format!("module-task-{done}")), "Done opened");
  assert_eq!(seeded.text("module-burndown-remaining"), "2 remaining");

  seeded.click("module-tab-burndown");
  assert_eq!(seeded.route(), burndown);
  seeded.until("the burndown", |seeded| seeded.shows("module-burndown"));
  assert!(seeded.text("module-burndown").contains("Counted in tasks"));
  assert!(seeded.shows("module-burndown-legend"), "the chart shows first");
  seeded.click("module-burndown-view-table");
  assert!(seeded.shows("module-burndown-table") && !seeded.shows("module-burndown-legend"));
  assert_eq!(
    rows(&mut seeded, "module-burndown-row-"),
    8,
    "three days back, today and four ahead"
  );
  let today_row = seeded.text("module-burndown-row-3");
  assert!(
    today_row.ends_with("3 2 3.0 — — 3 0"),
    "scope, remaining, ideal, the forecast's 50% and 85%, added, removed: {today_row}"
  );
  assert!(
    seeded
      .text("module-burndown-forecast")
      .contains("not enough history yet")
  );

  seeded.window.navigation().back();
  seeded.window.pass();
  assert_eq!(seeded.route(), summary, "Back returns to the Summary");
  seeded.until("the Summary", |seeded| seeded.shows("module-burndown-line"));
  seeded.window.press("module-burndown-line");
  assert_eq!(seeded.route(), burndown, "the burndown line opens the Burndown tab");
}

#[test]
fn modules_page_header_sits_where_the_task_page_header_does() {
  // The task page as `01 · Summary` draws it: its project has no module, so its header has no module field, and the
  // task is done, so no unavailable primary action says why under the title row while no runner serves the project.
  let mut plain = Seeded::new("tasks", |_| {});
  let done = plain
    .db()
    .tasks()
    .expect("tasks")
    .into_iter()
    .find(|task| task.organization().status() == WorkflowStatus::Done)
    .expect("the seed has a done task");
  let task = ProjectPage::Task(done.key().clone(), TaskView::Summary);
  plain.open(task, "task-summary-body");
  let places = |seeded: &mut Seeded, ids: [&str; 5]| ids.map(|id| seeded.bounds(id).y);
  let task_places = places(
    &mut plain,
    [
      "task-back-row",
      "task-title-row",
      "task-status-row",
      "task-tabs",
      "task-summary-body",
    ],
  );
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    picked = Some(module_with(db, details("Checkout", (3, 4)), &[(WorkflowStatus::Todo, 1)]).0);
  });
  let module = picked.expect("the module");
  seeded.open(ProjectPage::Module(module, ModuleTab::Summary), "module-summary-body");
  let module_places = places(
    &mut seeded,
    [
      "module-back-row",
      "module-header-title-row",
      "module-status-row",
      "module-tabs",
      "module-summary-body",
    ],
  );
  for (index, (task, module)) in task_places.iter().zip(module_places).enumerate() {
    assert!(
      (task - module).abs() < 0.5,
      "row {index}: the task page's at {task}, the module's at {module}"
    );
  }
  seeded.open(ProjectPage::Modules(Default::default()), "modules-header-title-row");
  assert!((seeded.bounds("modules-header-title-row").y - module_places[1]).abs() < 0.5);
  assert!((seeded.bounds("modules-tabs").y - module_places[3]).abs() < 0.5);
}

#[test]
fn modules_summary_properties_save_in_place() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    picked = Some(module_with(db, details("Checkout", (3, 4)), &[(WorkflowStatus::Todo, 1)]).0);
  });
  let module = picked.expect("the module");
  seeded.open(
    ProjectPage::Module(module, ModuleTab::Summary),
    "module-property-target",
  );
  assert!(seeded.text("module-property-created").starts_with("Created"));
  let facts = seeded.text("module-facts");
  seeded.click("module-property-target");
  seeded.until("the picker", |seeded| seeded.shows("module-property-target-picker"));
  let start = today().checked_sub_days(Days::new(3)).expect("a day");
  let before = start.pred_opt().expect("a day");
  let day = |seeded: &mut Seeded, day: chrono::NaiveDate| {
    let id = format!("module-property-target-picker-day-{day}");
    // The grid shows six weeks around the target's month: the month before or after holds the day otherwise.
    for key in ["PageUp", "PageDown", "PageDown"] {
      if seeded.shows(&id) {
        break;
      }
      seeded.key(key);
    }
    id
  };
  let id = day(&mut seeded, before);
  seeded.click(&id);
  assert!(
    seeded.shows("module-property-target-picker"),
    "a day before the start date cannot be the target"
  );
  let target = today().checked_add_days(Days::new(1)).expect("a day");
  let id = day(&mut seeded, target);
  seeded.click(&id);
  seeded.until("the target saved", |seeded| {
    seeded
      .module_named("Checkout")
      .and_then(|stored| stored.details().target())
      == Some(target)
  });
  seeded.until("the status line", |seeded| seeded.text("module-facts") != facts);
  seeded.click("module-property-state");
  seeded.until("the state menu", |seeded| seeded.shows("module-property-state-closed"));
  seeded.click("module-property-state-closed");
  seeded.until("the state saved", |seeded| {
    seeded.module_named("Checkout").map(|stored| stored.details().state()) == Some(ModuleState::Closed)
  });
  seeded.until("the chip", |seeded| seeded.text("module-state") == "Closed");
}

#[test]
fn modules_page_more_actions_close_and_reopen_and_new_task_starts_in_the_module() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    picked = Some(module_with(db, details("Checkout", (0, 7)), &[]).0);
  });
  let module = picked.expect("the module");
  seeded.open(ProjectPage::Module(module, ModuleTab::Summary), "module-more");
  assert_eq!(
    seeded.text("module-no-tasks"),
    "No task is in this module yet. Choose it as a task's module on the task form or the task's page."
  );
  seeded.click("module-more");
  seeded.until("the menu", |seeded| seeded.shows("module-close"));
  assert!(seeded.shows("module-edit"));
  seeded.click("module-close");
  seeded.until("closed", |seeded| seeded.text("module-state") == "Closed");
  seeded.click("module-more");
  seeded.until("the menu", |seeded| seeded.shows("module-reopen"));
  seeded.click("module-reopen");
  seeded.until("open again", |seeded| seeded.text("module-state") == "Open");
  seeded.click("module-more");
  seeded.until("the menu", |seeded| seeded.shows("module-edit"));
  seeded.window.press("module-edit");
  assert_eq!(
    seeded.route(),
    Route::Project(seeded.project, ProjectPage::EditModule(module))
  );

  seeded.open(ProjectPage::Module(module, ModuleTab::Summary), "module-new-task");
  seeded.window.press("module-new-task");
  assert_eq!(
    seeded.route(),
    Route::Project(seeded.project, ProjectPage::NewTaskInModule(module))
  );
  seeded.until("the form", |seeded| seeded.shows("task-form-module"));
  assert_eq!(
    seeded.text("task-form-module-value"),
    "Checkout",
    "the task starts in the module"
  );
}

#[test]
fn modules_page_says_when_the_module_is_gone() {
  let mut seeded = Seeded::new("tasks", |_| {});
  let gone = orchester_core::id::ModuleId::generate();
  seeded.open(ProjectPage::Module(gone, ModuleTab::Summary), "module-not-found");
  assert!(seeded.text("module-not-found").contains("This module no longer exists"));
  assert!(seeded.shows("module-back"), "the way back stays");
  let state = seeded.bounds("module-not-found");
  let middle = state.x + state.width / 2.0;
  for part in ["module-not-found-glyph", "module-not-found-title"] {
    let bounds = seeded.bounds(part);
    assert!(
      (bounds.x + bounds.width / 2.0 - middle).abs() < 1.0,
      "{part} is centered: {bounds:?} in {state:?}"
    );
  }
}

#[test]
fn modules_page_that_cannot_be_read_again_says_so() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    picked = Some(module_with(db, details("Checkout", (0, 7)), &[(WorkflowStatus::Todo, 1)]).0);
  });
  let module = picked.expect("the module");
  seeded.open(ProjectPage::Module(module, ModuleTab::Summary), "module-header");
  // The page cannot be read again: it says so instead of showing the module as read before.
  rusqlite::Connection::open(seeded.repo.join(".orchester").join("orchester.db"))
    .expect("the project database opens")
    .execute("ALTER TABLE module_changes RENAME TO module_changes_gone", [])
    .expect("the moves are out of reach");
  // A reading of the tasks under way since before the rename may stand for this one: ask again until a reading
  // made after it reaches the page.
  let mut asked = std::time::Instant::now();
  seeded.read_again();
  seeded.until("the read's failure", |seeded| {
    if asked.elapsed() > std::time::Duration::from_millis(200) {
      asked = std::time::Instant::now();
      seeded.read_again();
    }
    seeded.shows("module-load-error")
  });
  assert!(!seeded.shows("module-header") && !seeded.shows("module-more"));
  assert!(seeded.shows("module-back"), "the way back stays");
}
