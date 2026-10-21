//! The Modules screens (ORC-48) on seeded Kontur projects whose modules are made before the window opens: the list
//! with each module's progress opening its page; the page's progress, tasks and burndown as a chart or a table; the
//! editor making a module, saying why a module is refused, and refusing a change over one made meanwhile; a failed
//! read of the page shown as such, not as the page read before it; the module filter of Tasks; and the module field
//! of the task's header and of the task form. Needs `git`, as the seed does.

mod harness;
#[allow(
  dead_code,
  reason = "the seed's scenarios are shared with its own example; these tests use a part"
)]
#[path = "../../examples/seed/kontur/mod.rs"]
mod kontur;
#[path = "../support/mod.rs"]
mod support;

use harness::{Seeded, details, module_with};
use orchester_app::routes::{ProjectPage, Route, TaskList, TaskView};
use orchester_core::{
  module::{ModuleDetails, ModuleState},
  name::Name,
  task::organization::{ChangeActor, WorkflowStatus},
};

/// The rows of a table whose row ids are `<prefix><index>`.
fn rows(seeded: &mut Seeded, prefix: &str) -> usize {
  seeded
    .ids(prefix)
    .iter()
    .filter(|rest| rest.parse::<usize>().is_ok())
    .count()
}

#[test]
fn modules_list_shows_progress_and_the_page_its_tasks_and_burndown() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    let picks = [(WorkflowStatus::Done, 1), (WorkflowStatus::Todo, 2)];
    picked = Some(module_with(db, details("Checkout", (3, 4)), &picks));
  });
  let (module, keys) = picked.expect("the module");
  seeded.open(ProjectPage::Modules, "modules-row-0");
  let row = seeded.text("modules-row-0");
  for part in ["Checkout", "Open", "3", "33%"] {
    assert!(row.contains(part), "{part} in {row}");
  }
  assert_eq!(rows(&mut seeded, "modules-row-"), 1);
  seeded.window.press("modules-row-0");
  assert_eq!(
    seeded.route(),
    Route::Project(seeded.project, ProjectPage::Module(module))
  );
  seeded.until("the module's page", |seeded| seeded.shows("module-header"));
  assert_eq!(seeded.text("module-title"), "Checkout");
  let facts = seeded.text("module-facts");
  assert!(facts.contains("Open") && facts.contains("1 of 3 tasks done"), "{facts}");
  assert_eq!(rows(&mut seeded, "module-task-"), keys.len());
  assert!(seeded.text("module-burndown").contains("Counted in tasks"));
  assert!(seeded.shows("module-burndown-legend"), "the chart shows first");
  seeded.click("module-burndown-view-table");
  assert!(seeded.shows("module-burndown-table") && !seeded.shows("module-burndown-legend"));
  assert_eq!(
    rows(&mut seeded, "module-burndown-row-"),
    8,
    "three days back, today and four ahead"
  );
  let before = seeded.text("module-burndown-row-0");
  assert!(before.ends_with("— — — — —"), "no work yet three days ago: {before}");
  let today = seeded.text("module-burndown-row-3");
  assert!(
    today.ends_with("3 2 3.0 3 0"),
    "scope, remaining, ideal, added, removed: {today}"
  );
  let last = seeded.text("module-burndown-row-7");
  assert!(
    last.ends_with("— — 0.0 — —"),
    "a day to come has only its ideal: {last}"
  );
  seeded.click("module-burndown-view-chart");
  assert!(seeded.shows("module-burndown-legend") && !seeded.shows("module-burndown-table"));
}

#[test]
fn modules_editor_makes_a_module_and_says_why_one_is_refused() {
  let mut seeded = Seeded::new("tasks", |db| {
    module_with(db, details("Checkout", (0, 7)), &[]);
  });
  seeded.open(ProjectPage::Modules, "modules-row-0");
  seeded.click("modules-new");
  assert_eq!(seeded.text("module-editor-title"), "New module");
  seeded.set("module-name", " checkout ");
  seeded.set("module-start", "2026-13-01");
  seeded.click("module-editor-save");
  assert_eq!(seeded.text("module-start-note"), "Write the date as YYYY-MM-DD.");
  seeded.set("module-start", "2026-10-01");
  seeded.set("module-target", "2026-09-01");
  seeded.window.pass();
  assert!(!seeded.shows("module-start-note"));
  assert_eq!(
    seeded.text("module-target-note"),
    "The target date can't be before the start date."
  );
  seeded.set("module-target", "2026-10-31");
  seeded.click("module-editor-save");
  seeded.until("the refusal", |seeded| seeded.shows("module-editor-error"));
  assert_eq!(
    seeded.text("module-editor-error"),
    "Another module already has this name."
  );
  seeded.set("module-name", "Payments");
  seeded.click("module-editor-save");
  seeded.until("the new module", |seeded| {
    !seeded.shows("module-editor") && seeded.shows("modules-row-1")
  });
  let made = seeded.module_named("Payments").expect("Payments is stored");
  assert_eq!(
    made.details().target().map(|day| day.to_string()).as_deref(),
    Some("2026-10-31")
  );
  assert!(seeded.text("modules-row-1").contains("Payments"));
}

#[test]
fn modules_editor_refuses_a_change_over_one_made_meanwhile_and_a_failed_read_is_shown() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    picked = Some(module_with(db, details("Checkout", (0, 7)), &[(WorkflowStatus::Todo, 1)]).0);
  });
  let module = picked.expect("the module");
  seeded.open(ProjectPage::Module(module), "module-header");
  seeded.click("module-edit");
  assert_eq!(seeded.text("module-editor-title"), "Edit module");
  // Another app, or a tool, renames the module while the editor is open.
  let stored = seeded.module_named("Checkout").expect("stored");
  let renamed = ModuleDetails::new(
    Name::new("Checkout v2").expect("a name"),
    "",
    (stored.details().start(), stored.details().target()),
    ModuleState::Open,
  )
  .expect("details");
  seeded
    .db()
    .update_module(module, stored.details(), renamed)
    .expect("renamed elsewhere");
  seeded.set("module-name", "Checkout (closed)");
  seeded.click("module-editor-save");
  seeded.until("the refusal", |seeded| seeded.shows("module-editor-error"));
  assert!(
    seeded
      .text("module-editor-error")
      .starts_with("This module was changed since you opened it."),
    "{}",
    seeded.text("module-editor-error")
  );
  assert!(
    seeded.module_named("Checkout v2").is_some(),
    "the change made meanwhile stays"
  );
  // The page cannot be read again: it says so instead of showing the module as read before.
  rusqlite::Connection::open(seeded.repo.join(".orchester").join("orchester.db"))
    .expect("the project database opens")
    .execute("ALTER TABLE module_changes RENAME TO module_changes_gone", [])
    .expect("the moves are out of reach");
  seeded.read_again();
  seeded.until("the read's failure", |seeded| seeded.shows("module-load-error"));
  assert!(!seeded.shows("module-header") && !seeded.shows("module-edit"));
}

#[test]
fn modules_tasks_filter_keeps_the_tasks_of_a_module_or_of_none() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    let picks = [(WorkflowStatus::Todo, 2)];
    picked = Some(module_with(db, details("Checkout", (0, 7)), &picks).1);
  });
  let mut inside = picked.expect("the module's tasks");
  inside.sort();
  seeded.open(ProjectPage::Tasks(TaskList::default()), "tasks-module-filter");
  seeded.until("the tasks", |seeded| rows_of_tasks(seeded).len() == 24);
  assert_eq!(
    seeded.options("tasks-module-filter"),
    ["All modules", "No module", "Checkout"]
  );
  seeded.choose("tasks-module-filter", "Checkout");
  assert_eq!(rows_of_tasks(&mut seeded), inside);
  seeded.choose("tasks-module-filter", "No module");
  let outside = rows_of_tasks(&mut seeded);
  assert_eq!(outside.len(), 22);
  assert!(outside.iter().all(|key| !inside.contains(key)));
  seeded.choose("tasks-module-filter", "All modules");
  assert_eq!(rows_of_tasks(&mut seeded).len(), 24);
}

/// The keys of the Tasks list's rows, sorted.
fn rows_of_tasks(seeded: &mut Seeded) -> Vec<String> {
  seeded.ids("task-key-")
}

#[test]
fn modules_the_task_header_moves_the_task_as_a_person() {
  let mut picked = None;
  let mut seeded = Seeded::new("tasks", |db| {
    picked = Some(module_with(db, details("Checkout", (0, 7)), &[]).0);
  });
  let module = picked.expect("the module");
  let key = "KON-12";
  assert_eq!(seeded.task(key).module(), None);
  let page = ProjectPage::Task(key.parse().expect("a key"), TaskView::default());
  seeded.open(page, "task-module");
  assert_eq!(seeded.options("task-module"), ["No module", "Checkout"]);
  seeded.choose("task-module", "Checkout");
  seeded.until("the move", |seeded| seeded.task(key).module() == Some(module));
  let moves = seeded.db().module_changes(module).expect("the moves");
  let last = moves.last().expect("the move is kept");
  assert_eq!(
    (last.from, last.to, last.actor),
    (None, Some(module), ChangeActor::Person)
  );
  assert!(!seeded.shows("task-module-error"));
}

#[test]
fn modules_the_task_form_saves_the_chosen_module() {
  let mut picked = None;
  let mut seeded = Seeded::new("live", |db| {
    picked = Some(module_with(db, details("Checkout", (0, 7)), &[]).0);
  });
  let module = picked.expect("the module");
  seeded.open(
    ProjectPage::EditTask("KON-1".parse().expect("a key")),
    "task-form-module-input",
  );
  assert_eq!(seeded.options("task-form-module-input"), ["No module", "Checkout"]);
  seeded.choose("task-form-module-input", "Checkout");
  seeded.click("task-form-save");
  seeded.until("the save", |seeded| seeded.task("KON-1").module() == Some(module));
}
