//! Active's groups: Needs you holds exactly the tasks every *Needs you* names (the title bar's pill counts what they
//! wait for), every Active task is in one group, and what waits and is no task's (the project's memory records) is
//! named in a notice over the rows, whose *Open Needs you* opens the title bar's panel, which counts it. The Needs you
//! view, first, holds only what waits, on the list and on the board (ORC-105).

use std::collections::BTreeSet;

use orchester_app::routes::{TaskList, TaskListTab};

use crate::{board::press, harness::Seeded};

const BODY: &str = "This list shows tasks only. The Needs you panel lists everything that waits for you.";

#[test]
fn tasks_needs_you_names_exactly_the_title_bar_s_tasks_and_each_active_task_is_in_one_group() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  let tasks = seeded.shown_tasks();
  let waiting: BTreeSet<String> = tasks
    .tasks
    .iter()
    .filter(|task| tasks.waiting_tasks().contains(&task.id()))
    .map(|task| task.key().to_string())
    .collect();
  assert!(!waiting.is_empty(), "the seed has tasks that wait for the person");
  let groups = seeded.groups();
  let names: Vec<&str> = groups.iter().map(|(name, _)| name.as_str()).collect();
  assert_eq!(names.first(), Some(&"needs_you"), "Needs you comes first: {names:?}");
  let order = ["needs_you", "running", "queued", "ready", "stopped", "finished"];
  let mut at = 0;
  for name in &names {
    let found = order[at..].iter().position(|known| known == name);
    at += found.unwrap_or_else(|| panic!("{name} is out of order or unknown: {names:?}"));
  }
  let needs_you: BTreeSet<String> = groups[0].1.iter().cloned().collect();
  assert_eq!(needs_you, waiting, "Needs you holds the tasks every Needs you names");
  assert_eq!(seeded.text("tasks-group-needs_you-count"), waiting.len().to_string());
  let pill = seeded.window.texts("needs-you-pill").expect("the pill").join(" ");
  assert!(
    pill.starts_with(&tasks.needs_you().to_string()),
    "the pill counts what they wait for: {pill}"
  );
  let shown: Vec<String> = groups.iter().flat_map(|(_, keys)| keys.clone()).collect();
  let once: BTreeSet<&String> = shown.iter().collect();
  assert_eq!(shown.len(), once.len(), "no task is in two groups: {groups:?}");
  assert_eq!(shown.len(), 18, "every Active task is in a group: {groups:?}");
  assert_eq!(seeded.view("active"), "Active 18");
}

#[test]
fn tasks_all_is_grouped_by_workflow_status() {
  let mut seeded = Seeded::tasks("tasks", TaskList::new(TaskListTab::All));
  let groups = seeded.groups();
  let counted: Vec<(&str, usize)> = groups.iter().map(|(name, keys)| (name.as_str(), keys.len())).collect();
  assert_eq!(
    counted,
    [
      ("backlog", 3),
      ("todo", 8),
      ("in_progress", 8),
      ("in_review", 2),
      ("done", 2),
      ("canceled", 1)
    ]
  );
}

#[test]
fn tasks_a_group_folds_away_under_its_header() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  let queued = seeded
    .groups()
    .into_iter()
    .find(|(name, _)| name == "queued")
    .expect("a Queued group");
  let key = queued.1[0].clone();
  seeded.click("tasks-group-queued");
  seeded.until("the group folded", |seeded| !seeded.shows(&format!("task-row-{key}")));
  assert_eq!(
    seeded.text("tasks-group-queued-count"),
    queued.1.len().to_string(),
    "its header keeps its count"
  );
  seeded.click("tasks-group-queued");
  seeded.until("the group open again", |seeded| {
    seeded.shows(&format!("task-row-{key}"))
  });
}

#[test]
fn tasks_needs_you_with_only_memory_waiting_names_the_memory_over_the_rows() {
  let mut seeded = Seeded::tasks("memory-only", TaskList::default());
  assert!(!seeded.shows("tasks-group-needs_you"), "no task waits");
  assert_eq!(
    seeded.texts("tasks-outside-tasks"),
    ["1 memory record waits for approval", BODY, "Open Needs you"]
  );
  assert_eq!(
    seeded.window.texts("needs-you-pill"),
    Some(vec!["1 needs you".to_owned()]),
    "the pill counts the record"
  );
  assert!(!seeded.shows("needs-you-head"));
  seeded.click("tasks-outside-tasks-open");
  assert_eq!(
    seeded.text("needs-you-count"),
    "1",
    "the panel opens and lists the record"
  );
}

#[test]
fn tasks_needs_you_lists_the_waiting_tasks_and_names_the_memory_over_them() {
  let mut seeded = Seeded::tasks("tasks-memory", TaskList::default());
  assert_eq!(seeded.text("tasks-group-needs_you-count"), "6");
  assert_eq!(
    seeded.texts("tasks-outside-tasks"),
    ["2 memory records wait for approval", BODY, "Open Needs you"]
  );
  assert_eq!(
    seeded.window.texts("needs-you-pill"),
    Some(vec!["7 need you".to_owned()]),
    "the pill counts the tasks' waits and the records"
  );
  let (views, notice, list) = (
    seeded.bounds("tasks-views"),
    seeded.bounds("tasks-outside-tasks"),
    seeded.bounds("tasks-list"),
  );
  assert!(views.y + views.height <= notice.y, "the notice is under the views");
  assert!(notice.y + notice.height <= list.y, "the notice is over the rows");
  seeded.go(TaskList::new(TaskListTab::NeedsYou));
  seeded.until("the Needs you view", |seeded| seeded.rows_laid_out());
  assert!(seeded.shows("tasks-outside-tasks"), "Needs you names it too");
  seeded.go(TaskList::new(TaskListTab::All));
  assert!(
    !seeded.shows("tasks-outside-tasks"),
    "only Needs you and Active name it"
  );
}

/// The keys of the seed's tasks something of waits for the person.
fn waiting(seeded: &Seeded) -> BTreeSet<String> {
  let tasks = seeded.shown_tasks();
  tasks
    .tasks
    .iter()
    .filter(|task| tasks.waiting_tasks().contains(&task.id()))
    .map(|task| task.key().to_string())
    .collect()
}

#[test]
fn tasks_needs_you_is_the_first_view_and_holds_only_what_waits() {
  let mut seeded = Seeded::tasks("tasks", TaskList::new(TaskListTab::NeedsYou));
  let waiting = waiting(&seeded);
  let views: Vec<String> = seeded
    .ids("tasks-view-")
    .into_iter()
    .filter(|id| !id.trim_start_matches("tasks-view-").contains('-'))
    .collect();
  assert_eq!(
    views,
    [
      "tasks-view-needs_you",
      "tasks-view-active",
      "tasks-view-backlog",
      "tasks-view-done",
      "tasks-view-all"
    ],
    "Needs you comes first"
  );
  assert_eq!(seeded.view("needs_you"), format!("Needs you {}", waiting.len()));
  assert!(seeded.shows("tasks-view-needs_you-icon"), "its hand");
  assert!(seeded.groups().is_empty(), "Needs you is flat");
  let shown: BTreeSet<String> = seeded.keys().into_iter().collect();
  assert_eq!(shown, waiting, "only the tasks that wait");
}

#[test]
fn tasks_needs_you_board_shows_only_the_waiting_cards() {
  let mut seeded = Seeded::board("tasks", TaskList::new(TaskListTab::NeedsYou));
  let waiting = waiting(&seeded);
  let tasks = seeded.shown_tasks();
  for task in tasks.tasks.iter() {
    let key = task.key().to_string();
    let card = format!("task-key-{key}");
    assert_eq!(
      seeded.shows(&card),
      waiting.contains(&key),
      "{key} is on the board only while it waits"
    );
  }
  assert!(
    !seeded.shows("tasks-board-column-backlog"),
    "a column without a waiting card stays away"
  );
}

/// Shows the board's rows by `lanes` through Display.
fn rows_by(seeded: &mut Seeded, lanes: &str) {
  press(seeded, "tasks-display");
  press(seeded, "tasks-board-options-rows");
  press(seeded, &format!("tasks-board-options-rows-{lanes}"));
  seeded
    .window
    .tree
    .key_down("Escape".to_owned(), "Escape".to_owned(), false, false, false);
  seeded.until("Display closed", |seeded| !seeded.shows("tasks-board-options"));
}

/// Needs you on the board when nothing waits: the empty view's words, not a board without columns.
fn says_needs_you_is_empty(seeded: &mut Seeded, lanes: &str) {
  seeded.go(TaskList::new(TaskListTab::NeedsYou));
  seeded.until("the empty view", |seeded| seeded.laid_out("tasks-no-rows-title"));
  assert_eq!(
    seeded.text("tasks-no-rows-title"),
    "No tasks in this view.",
    "rows by {lanes}"
  );
  assert!(!seeded.shows("tasks-board-columns"), "no blank board, rows by {lanes}");
  assert!(!seeded.shows("tasks-board-lanes"), "no blank rows, rows by {lanes}");
  assert!(seeded.shows("tasks-outside-tasks"), "the memory record is named");
}

#[test]
fn tasks_needs_you_board_with_nothing_waiting_says_so_with_rows_too() {
  // The seed's one task is done: All has a column, Needs you none.
  let mut seeded = Seeded::board("memory-only", TaskList::new(TaskListTab::All));
  says_needs_you_is_empty(&mut seeded, "none");
  seeded.go(TaskList::new(TaskListTab::All));
  seeded.until("the board", |seeded| seeded.laid_out("tasks-board-columns"));
  rows_by(&mut seeded, "module");
  seeded.until("the rows", |seeded| seeded.laid_out("tasks-board-lanes"));
  says_needs_you_is_empty(&mut seeded, "module");
}

/// ORC-150, as OP-26 was left: a task set Done while its report run waited to be completed is counted in Done and never
/// in Needs you or Active, and the app settles the run's completion as the person's, who closed it.
#[test]
fn tasks_a_task_closed_while_its_report_waited_is_in_done_only_and_its_completion_settles() {
  let mut seeded = Seeded::tasks("deliverables", TaskList::new(TaskListTab::Done));
  let tasks = seeded.shown_tasks();
  let closed = tasks
    .tasks
    .iter()
    .find(|task| task.details().title.as_str() == "Summarize the outage")
    .expect("the closed task");
  let (id, key) = (closed.id(), closed.key().to_string());
  let run = tasks
    .phases
    .get(&id)
    .and_then(|latest| latest.run())
    .expect("its run")
    .id;
  let db = orchester_store::ProjectDb::open(&seeded.repo).expect("the project");
  seeded.until("the completion settled", |_| {
    db.run_completion(run).expect("read").is_some()
  });
  let completion = db.run_completion(run).expect("read").expect("settled");
  assert_eq!(completion.by, orchester_core::run::CompletedBy::Person);
  seeded.until("the tasks read again", |seeded| {
    seeded.view("needs_you") == "Needs you 3"
  });
  assert_eq!(
    [seeded.view("active"), seeded.view("done")],
    ["Active 3", "Done 2"],
    "the completed report and the closed one are in Done only"
  );
  assert!(seeded.keys().contains(&key), "{key} is in Done");
  assert!(!waiting(&seeded).contains(&key), "{key} waits for nothing");
}
