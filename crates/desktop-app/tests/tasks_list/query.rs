//! The views with their counts, the search, the Filter with its chips, and the sorts in Display.

use orchester_app::routes::{TaskList, TaskListTab};

use crate::harness::Seeded;

fn search(text: &str) -> TaskList {
  TaskList::new(TaskListTab::All).with_search(text)
}

#[test]
fn tasks_views_count_active_backlog_done_and_all_each_task_once() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  assert_eq!(
    ["active", "backlog", "done", "all"].map(|view| seeded.view(view)),
    ["Active 18", "Backlog 3", "Done 3", "All 24"],
    "Active is Todo, In progress and In review; Done holds Done and Canceled"
  );
  let mut every = Vec::new();
  for tab in [TaskListTab::Active, TaskListTab::Backlog, TaskListTab::Done] {
    seeded.go(TaskList::new(tab));
    seeded.until("the rows laid out", Seeded::rows_laid_out);
    every.extend(seeded.keys());
  }
  every.sort();
  let mut all = (1..=24).map(|number| format!("KON-{number}")).collect::<Vec<_>>();
  all.sort();
  assert_eq!(every, all, "each task is in one of Active, Backlog and Done");
  seeded.go(search("Retry Slack notifications"));
  assert_eq!(seeded.keys(), ["KON-7"], "All holds the Canceled task too");
  assert_eq!(seeded.view("done"), "Done 1", "the counts follow the search");
  assert_eq!(seeded.view("active"), "Active 0");
}

#[test]
fn tasks_search_finds_the_title_key_brief_and_former_key() {
  let mut seeded = Seeded::tasks("tasks", TaskList::new(TaskListTab::All));
  for (text, keys) in [
    ("webhook sender", vec!["KON-12"]),
    ("kon-24", vec!["KON-24"]),
    ("both keys work for an hour", vec!["KON-5"]),
  ] {
    seeded.go(search(text));
    assert_eq!(seeded.keys(), keys, "{text}");
  }
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
  assert_eq!(
    seeded.window.navigation().route_now(),
    seeded.route(TaskList::default().with_search("HubSpot"))
  );
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
fn tasks_slash_reaches_the_search() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  assert_eq!(seeded.text("tasks-search-keys"), "/", "the key cap says how");
  seeded
    .window
    .tree
    .key_down("/".to_owned(), "Slash".to_owned(), false, false, false);
  seeded.until("the search focused", |seeded| {
    seeded.window.tree.focused_element().and_then(|element| element.id()) == Some("tasks-search")
  });
}

#[test]
fn tasks_filter_narrows_the_rows_by_pipeline_and_priority_and_its_chips_let_them_go() {
  let mut seeded = Seeded::tasks("tasks", TaskList::new(TaskListTab::All));
  assert_eq!(seeded.text("tasks-filter"), "Filter");
  seeded.click("tasks-filter");
  seeded.until("the filter menu", |seeded| seeded.laid_out("tasks-filter-menu"));
  let menu = seeded.texts("tasks-filter-menu");
  for word in [
    "Pipeline",
    "Feature delivery v3",
    "Quick fix v2",
    "No pipeline",
    "Priority",
    "High",
    "Tag",
    "webhooks",
  ] {
    assert!(menu.iter().any(|said| said == word), "the menu offers {word}: {menu:?}");
  }
  assert!(
    !menu.iter().any(|said| said == "Module"),
    "the project has no module: {menu:?}"
  );
  let quick = option(&mut seeded, "pipeline", "Quick fix v2");
  seeded.click(&quick);
  seeded.until("the rows laid out", Seeded::rows_laid_out);
  let keys = seeded.keys();
  assert!(!keys.is_empty() && keys.len() < 24, "{keys:?}");
  for key in &keys {
    let pipeline = seeded.text(&format!("task-pipeline-{key}"));
    assert_eq!(pipeline, "Quick fix v2", "{key}");
  }
  assert_eq!(seeded.text("tasks-filter"), "Filter · 1");
  let high = option(&mut seeded, "priority", "High");
  seeded.click(&high);
  seeded.until("the rows laid out", Seeded::rows_laid_out);
  assert!(
    seeded.keys().is_empty(),
    "no Quick fix task is High: {:?}",
    seeded.keys()
  );
  assert_eq!(seeded.text("tasks-filter"), "Filter · 2");
  assert_eq!(seeded.text("tasks-chip-pipeline-1"), "Pipeline Quick fix v2");
  assert_eq!(seeded.text("tasks-chip-priority-1"), "Priority High");
  assert!(
    seeded.shows("tasks-clear-query"),
    "the view says the filters hide its rows"
  );
  // The menu closes with its button; it stays open while options are chosen.
  seeded.click("tasks-filter");
  seeded.until("the menu closed", |seeded| !seeded.shows("tasks-filter-menu"));
  seeded.click("tasks-chip-pipeline-1-remove");
  seeded.until("the High tasks", |seeded| !seeded.keys().is_empty());
  let mut high = seeded.keys();
  high.sort();
  assert_eq!(high, ["KON-10", "KON-12", "KON-18", "KON-5"], "the High tasks");
  seeded.click("tasks-filter-clear");
  seeded.until("every row", |seeded| seeded.keys().len() == 24);
  assert!(!seeded.shows("tasks-filters"), "no chip once nothing is chosen");
  assert_eq!(seeded.text("tasks-filter"), "Filter");
}

#[test]
fn tasks_filter_lets_a_chosen_tag_go_once_the_project_drops_it() {
  let mut seeded = Seeded::tasks("tasks", TaskList::new(TaskListTab::All));
  seeded.click("tasks-filter");
  seeded.until("the filter menu", |seeded| seeded.laid_out("tasks-filter-menu"));
  // The menu's search brings the tag, under the options' fold, into view.
  seeded
    .window
    .tree
    .get_element_by_id_mut("tasks-filter-search")
    .and_then(|element| element.as_text_input())
    .expect("the menu's search")
    .set_value("webhooks");
  seeded.until("the tag found", |seeded| seeded.laid_out("tasks-filter-tag-0"));
  let webhooks = option(&mut seeded, "tag", "webhooks");
  seeded.click(&webhooks);
  seeded.until("the rows laid out", Seeded::rows_laid_out);
  assert_eq!(seeded.text("tasks-filter"), "Filter · 1");
  let keys = seeded.keys();
  assert!(keys.len() < 24, "only the webhooks tasks: {keys:?}");
  let mut db = orchester_store::ProjectDb::open(&seeded.repo).expect("the project database opens");
  let tag = db
    .project_tags()
    .expect("tags")
    .into_iter()
    .find(|tag| tag.name().as_str() == "webhooks")
    .expect("the webhooks tag");
  db.delete_project_tag(tag.id(), orchester_core::time::Timestamp::now())
    .expect("the tag is deleted");
  seeded.window.projects().feed(seeded.project).saved();
  // A value no chip could let go no longer filters, nor counts.
  seeded.until("every row", |seeded| seeded.keys().len() == 24);
  assert_eq!(seeded.text("tasks-filter"), "Filter");
  assert!(!seeded.shows("tasks-filters"), "no chip once nothing is chosen");
}

/// The id of the option of `facet` saying `words` in the open Filter menu.
fn option(seeded: &mut Seeded, facet: &str, words: &str) -> String {
  (0..32)
    .map(|index| format!("tasks-filter-{facet}-{index}"))
    .find(|id| seeded.window.texts(id).is_some_and(|said| said.join(" ") == words))
    .unwrap_or_else(|| panic!("the {facet} facet offers {words}"))
}

#[test]
fn tasks_display_orders_the_rows_and_says_sorting_leaves_the_queue_alone() {
  let mut seeded = Seeded::tasks("tasks", TaskList::new(TaskListTab::Backlog));
  assert_eq!(seeded.keys(), ["KON-3", "KON-4", "KON-22"], "manual order");
  sort(&mut seeded, "newest");
  assert_eq!(seeded.keys(), ["KON-22", "KON-4", "KON-3"]);
  sort(&mut seeded, "priority");
  assert_eq!(
    seeded.keys().first().map(String::as_str),
    Some("KON-3"),
    "Low before no priority"
  );
  seeded.click("tasks-display");
  seeded.until("the display menu", |seeded| seeded.laid_out("tasks-sort-note"));
  assert_eq!(seeded.text("tasks-sort-note"), "Sorting never changes the run queue.");
}

/// Chooses the order `name` in Display.
fn sort(seeded: &mut Seeded, name: &str) {
  seeded.click("tasks-display");
  let id = format!("tasks-sort-{name}");
  seeded.until("the display menu", |seeded| seeded.laid_out(&id));
  seeded.click(&id);
  seeded.until("the rows laid out", Seeded::rows_laid_out);
}

#[test]
fn tasks_display_and_more_menus_drop_down_under_their_buttons_not_across_the_window() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  for (button, menu) in [
    ("tasks-display", "tasks-display-menu"),
    ("tasks-more", "tasks-more-menu"),
  ] {
    seeded.click(button);
    seeded.until("the menu", |seeded| seeded.laid_out(menu));
    let (anchor, panel) = (seeded.bounds(button), seeded.bounds(menu));
    assert_eq!(panel.width, 260.0, "{menu} keeps the Filter menu's width");
    assert!(
      panel.x <= anchor.x + anchor.width && panel.x + panel.width >= anchor.x,
      "{menu} drops down under {button}: {panel:?} against {anchor:?}"
    );
    seeded.click(button);
    seeded.until("the menu closed", |seeded| !seeded.shows(menu));
  }
}

#[test]
fn tasks_the_search_s_cross_clears_it() {
  let mut seeded = Seeded::tasks("tasks", search("HubSpot"));
  assert_eq!(seeded.keys(), ["KON-13"]);
  seeded.click("tasks-search-clear");
  assert_eq!(
    seeded.window.navigation().route_now(),
    seeded.route(TaskList::new(TaskListTab::All))
  );
  assert_eq!(seeded.keys().len(), 24);
  assert!(!seeded.shows("tasks-search-clear"), "an empty search has no cross");
}
