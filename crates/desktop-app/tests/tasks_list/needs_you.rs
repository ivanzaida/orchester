//! Active's groups: Needs you holds exactly the tasks every *Needs you* names (the title bar's pill counts what they
//! wait for), every Active task is in one group, and what waits and is no task's (the project's memory records) is
//! named in a notice over the rows, whose *Open Needs you* opens the title bar's panel, which counts it.

use std::collections::BTreeSet;

use orchester_app::routes::{TaskList, TaskListTab};

use crate::harness::Seeded;

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
  let counted: Vec<(&str, usize)> = groups
    .iter()
    .map(|(name, keys)| (name.as_str(), keys.len()))
    .collect();
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
  seeded.until("the group open again", |seeded| seeded.shows(&format!("task-row-{key}")));
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
  seeded.go(TaskList::new(TaskListTab::All));
  assert!(!seeded.shows("tasks-outside-tasks"), "only Active names it");
}
