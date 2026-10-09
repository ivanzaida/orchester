//! The workflow tabs with their counts, the search, the pipeline filter and the sorts.

use orchester_app::routes::{TaskList, TaskListTab};

use crate::harness::Seeded;

fn search(text: &str) -> TaskList {
  TaskList::new(TaskListTab::All).with_search(text)
}

#[test]
fn tasks_tabs_count_each_workflow_status_and_canceled_only_under_all() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  let labels: Vec<String> = TaskListTab::ALL
    .into_iter()
    .map(|tab| seeded.text(&format!("tasks-tab-{}-label", tab.name())))
    .collect();
  assert_eq!(
    labels,
    [
      "All 24",
      "Needs you 4",
      "Backlog 3",
      "Todo 8",
      "In progress 8",
      "In review 2",
      "Done 2"
    ]
  );
  assert_eq!(seeded.keys().len(), 24, "All holds every task, the Canceled one too");
  let canceled = "Retry Slack notifications";
  seeded.go(search(canceled));
  assert_eq!(seeded.keys(), ["KON-7"]);
  for tab in TaskListTab::ALL.into_iter().skip(1) {
    seeded.go(TaskList::new(tab).with_search(canceled));
    assert!(seeded.keys().is_empty(), "{tab:?} leaves the Canceled task out");
    assert!(seeded.shows("tasks-no-rows"), "{tab:?}");
  }
}

#[test]
fn tasks_search_finds_the_title_key_brief_and_former_key() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  for (text, keys) in [
    ("webhook sender", vec!["KON-12"]),
    ("kon-24", vec!["KON-24"]),
    ("both keys work for an hour", vec!["KON-5"]),
  ] {
    seeded.go(search(text));
    assert_eq!(seeded.keys(), keys, "{text}");
  }
  assert!(
    seeded.text("tasks-footer").starts_with("1 task matches"),
    "{}",
    seeded.text("tasks-footer")
  );
  let changed = rusqlite::Connection::open(seeded.repo.join(".orchester").join("orchester.db"))
    .expect("the project database opens")
    .execute("UPDATE tasks SET former_key = 'ATL-7' WHERE number = 9", [])
    .expect("the former key is written");
  assert_eq!(changed, 1);
  seeded.window.projects().feed(seeded.project).saved();
  seeded.go(search("atl-7"));
  seeded.until("the former key", |seeded| seeded.keys() == ["KON-9"]);
}

#[test]
fn tasks_typing_a_search_takes_the_entry_s_place_and_back_leaves_the_list() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  seeded
    .window
    .tree
    .get_element_by_id_mut("tasks-search")
    .and_then(|element| element.as_text_input())
    .expect("the search is a text input")
    .set_value("HubSpot");
  seeded.window.pass();
  assert_eq!(seeded.window.navigation().route_now(), seeded.route(search("HubSpot")));
  assert_eq!(seeded.keys(), ["KON-13"]);
  seeded.window.navigation().back();
  seeded.window.pass();
  assert!(
    !matches!(
      seeded.window.navigation().route_now(),
      orchester_app::routes::Route::Project(_, orchester_app::routes::ProjectPage::Tasks(_))
    ),
    "the search replaced the list's entry, so Back leaves the list"
  );
}

#[test]
fn tasks_pipeline_filter_narrows_the_rows_and_sorts_order_them() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  assert_eq!(seeded.keys().first().map(String::as_str), Some("KON-1"), "manual order");
  choose(&mut seeded, "tasks-sort", "Newest first");
  let newest = seeded.keys();
  assert_eq!(
    (newest.first().map(String::as_str), newest.last().map(String::as_str)),
    (Some("KON-24"), Some("KON-1"))
  );
  choose(&mut seeded, "tasks-sort", "Title");
  assert_eq!(
    seeded.keys().first().map(String::as_str),
    Some("KON-12"),
    "Add retry to webhook sender"
  );
  choose(&mut seeded, "tasks-sort", "Workflow status");
  let by_status = seeded.keys();
  let mut backlog = by_status[..3].to_vec();
  backlog.sort();
  assert_eq!(backlog, ["KON-22", "KON-3", "KON-4"], "Backlog first: {by_status:?}");
  assert_eq!(
    by_status.last().map(String::as_str),
    Some("KON-7"),
    "Canceled last: {by_status:?}"
  );
  choose(&mut seeded, "tasks-sort", "Priority");
  let by_priority = seeded.keys();
  let group = |range: std::ops::Range<usize>| {
    let mut keys = by_priority[range].to_vec();
    keys.sort();
    keys
  };
  assert_eq!(
    group(0..4),
    ["KON-10", "KON-12", "KON-18", "KON-5"],
    "High first: {by_priority:?}"
  );
  assert_eq!(group(22..24), ["KON-22", "KON-4"], "No priority last: {by_priority:?}");
  choose(&mut seeded, "tasks-pipeline-filter", "Quick fix");
  let keys = seeded.keys();
  assert!(!keys.is_empty() && keys.len() < 24, "{keys:?}");
  for key in &keys {
    let pipeline = seeded.text(&format!("task-pipeline-{key}"));
    assert!(pipeline.starts_with("Quick fix"), "{key}: {pipeline}");
  }
  let footer = seeded.text("tasks-footer");
  assert!(footer.starts_with(&format!("{} tasks match", keys.len())), "{footer}");
}

/// Chooses the option of the select `id` whose label starts with `label`.
fn choose(seeded: &mut Seeded, id: &str, label: &str) {
  let mut select = seeded
    .window
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.as_select())
    .unwrap_or_else(|| panic!("{id} is a select"));
  let index = select
    .labels()
    .iter()
    .position(|option| option.starts_with(label))
    .unwrap_or_else(|| panic!("{id} offers {label}: {:?}", select.labels()));
  select.commit(index);
  seeded.window.pass();
}

#[test]
fn tasks_the_search_s_cross_clears_it() {
  let mut seeded = Seeded::tasks("tasks", search("HubSpot"));
  assert_eq!(seeded.keys(), ["KON-13"]);
  seeded.click("tasks-search-clear");
  assert_eq!(
    seeded.window.navigation().route_now(),
    seeded.route(TaskList::default())
  );
  assert_eq!(seeded.keys().len(), 24);
  assert!(!seeded.shows("tasks-search-clear"), "an empty search has no cross");
}
