//! What each row says about its task in one phrase, in the tone the design gives it, what the pointer brings up on it,
//! and where a row, its action, New task and a Create's highlight lead.

use orchester_app::{
  routes::{ProjectPage, Route, TaskList, TaskListTab, TaskTab, TaskView},
  ui::icon::Icon,
};

use crate::harness::Seeded;

/// A row of the seed's `tasks`: its key, what its phrase starts with and its status icon. No runner serves the
/// project, so a run recorded as running is interrupted.
const ROWS: [(&str, &str, Icon); 13] = [
  ("KON-1", "Merged into", Icon::GitMerge),
  ("KON-3", "Needs ", Icon::PencilLine),
  ("KON-5", "Ready to queue", Icon::Circle),
  ("KON-8", "Waiting for its run to start", Icon::Clock3),
  ("KON-10", "Review plan v1", Icon::Map),
  ("KON-11", "Failed at 0", Icon::CircleX),
  ("KON-12", "Approve cargo add rand", Icon::Hand),
  ("KON-14", "Stopped after retries", Icon::CirclePause),
  ("KON-16", "No checkpoint yet", Icon::CirclePause),
  ("KON-17", "Held · a task failed", Icon::CirclePause),
  ("KON-19", "Blocked · KON-18 unfinished", Icon::Ban),
  ("KON-23", "Stopped at its token limit", Icon::Gauge),
  ("KON-24", "Latest run unreadable", Icon::CircleHelp),
];

#[test]
fn tasks_each_row_says_one_state_phrase_with_its_icon() {
  let mut seeded = Seeded::tasks("tasks", TaskList::new(TaskListTab::All));
  for (key, phrase, mark) in ROWS {
    let said = seeded.text(&format!("task-state-{key}"));
    assert!(said.starts_with(phrase), "{key}: {said}");
    assert_eq!(
      seeded.glyph(&format!("task-status-{key}")),
      mark.glyph(),
      "{key}: the mark of {said}"
    );
  }
  let failed = seeded.text("task-state-KON-11");
  assert!(
    failed.ends_with(" Run checks"),
    "a failed run says at which step: {failed}"
  );
}

#[test]
fn tasks_a_row_shows_its_key_title_tags_pipeline_priority_and_age() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  assert_eq!(seeded.texts("task-key-KON-12"), ["KON-12"]);
  assert_eq!(
    seeded.texts("task-title-KON-12"),
    ["Add retry to webhook sender", "backend", "webhooks"],
    "the tags in the project's order"
  );
  assert_eq!(seeded.text("task-pipeline-KON-12"), "Feature delivery v3");
  assert!(seeded.shows("task-priority-KON-12"), "the priority's icon");
  let age = seeded.text("task-age-KON-12");
  let (count, unit) = age.split_at(age.len() - 1);
  assert!(
    count.parse::<u32>().is_ok() && ["m", "h", "d"].contains(&unit),
    "its age in its largest whole unit: {age}"
  );
  assert!(!seeded.shows("task-new-KON-12"), "only a Create's task is new");
  assert!(
    !seeded.shows("task-more-KON-12"),
    "the actions show under the pointer only"
  );
  assert!(!seeded.shows("tasks-footer"), "no footer");
}

#[test]
fn tasks_the_pointer_brings_up_a_waiting_task_s_action_and_its_menu() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  seeded.hover("task-row-KON-10");
  seeded.until("the row's actions", |seeded| seeded.laid_out("task-more-KON-10"));
  assert!(
    !seeded.shows("task-pipeline-KON-10"),
    "the actions take the pipeline's place"
  );
  assert_eq!(seeded.text("task-action-KON-10"), "Review plan");
  seeded.click("task-action-KON-10");
  let summary = ProjectPage::Task("KON-10".parse().expect("a task key"), TaskView::of(TaskTab::Summary));
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, summary),
    "the plan is decided on its Summary"
  );
  seeded.go(TaskList::default());
  seeded.until("the rows laid out", Seeded::rows_laid_out);
  seeded.hover("task-row-KON-5");
  seeded.until("the row's menu", |seeded| seeded.laid_out("task-more-KON-5"));
  assert!(
    !seeded.shows("task-action-KON-5"),
    "a task nothing waits for has its menu only"
  );
}

#[test]
fn tasks_a_row_opens_the_task_s_details_and_new_task_opens_the_form() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  seeded.click("task-row-KON-9");
  let details = ProjectPage::Task("KON-9".parse().expect("a task key"), TaskView::of(TaskTab::Summary));
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, details)
  );
  seeded.go(TaskList::default());
  seeded.click("tasks-new-task");
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::NewTask)
  );
}

#[test]
fn tasks_highlight_tints_the_task_a_create_added_and_badges_it_new() {
  let key = "KON-22".parse().expect("a task key");
  let mut seeded = Seeded::tasks("tasks", TaskList::new(TaskListTab::All).with_highlight(key));
  assert_eq!(seeded.text("task-new-KON-22"), "New");
  let tinted = |seeded: &mut Seeded, key: &str| {
    seeded
      .window
      .tree
      .get_element_by_id(&format!("task-row-{key}"))
      .is_some_and(|row| row.children().iter().any(|line| line.has_class("highlighted")))
  };
  assert!(tinted(&mut seeded, "KON-22"));
  assert!(!tinted(&mut seeded, "KON-21"));
  assert!(!seeded.shows("task-new-KON-21"));
}

#[test]
fn tasks_highlight_below_the_fold_scrolls_its_row_into_view() {
  let key = "KON-7".parse().expect("a task key");
  let mut seeded = Seeded::tasks_in("tasks", TaskList::new(TaskListTab::All).with_highlight(key), 900);
  seeded.until("the new task in view", |seeded| seeded.in_view("KON-7"));
  assert!(
    !seeded.in_view("KON-3"),
    "the list scrolled to it: Backlog's KON-3 is above the fold"
  );
}

#[test]
fn tasks_another_view_sort_or_search_starts_the_list_at_its_top() {
  let mut seeded = Seeded::tasks_in("tasks", TaskList::new(TaskListTab::All), 900);
  seeded.scroll_down();
  assert!(!seeded.in_view("KON-3"), "the list scrolled down");
  seeded.go(TaskList::new(TaskListTab::All).with_search("e"));
  seeded.until("a new search to start at the top", |seeded| {
    let first = seeded.keys()[0].clone();
    seeded.in_view(&first)
  });
  seeded.scroll_down();
  seeded.go(TaskList::default());
  seeded.until("a new view to start at the top", |seeded| {
    let first = seeded.keys()[0].clone();
    seeded.in_view(&first)
  });
}

#[test]
fn tasks_a_created_task_the_feed_reads_later_is_scrolled_into_view() {
  let key = "KON-25".parse().expect("a task key");
  let mut seeded = Seeded::tasks_in("tasks", TaskList::new(TaskListTab::All).with_highlight(key), 900);
  assert!(
    !seeded.shows("task-row-KON-25"),
    "the feed has not read the new task yet"
  );
  assert!(seeded.in_view("KON-3"), "the rows wait at their top");
  let details = orchester_core::task::TaskDetails {
    title: orchester_core::name::Name::new("Show token spend per step").expect("a title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: orchester_core::task::TaskLimits::NONE,
  };
  orchester_workspace::tasks::data::save_draft_in(
    &seeded.repo,
    orchester_workspace::tasks::data::SaveTarget::New(orchester_core::id::IdempotencyKey::generate()),
    &details,
  )
  .expect("the draft is saved");
  seeded.window.projects().feed(seeded.project).saved();
  seeded.until("the new task in view", |seeded| seeded.in_view("KON-25"));
  assert_eq!(seeded.text("task-new-KON-25"), "New");
}

#[test]
fn tasks_rows_that_come_back_after_a_search_without_matches_start_at_the_top() {
  let mut seeded = Seeded::tasks_in("tasks", TaskList::new(TaskListTab::All), 900);
  seeded.scroll_down();
  assert!(!seeded.in_view("KON-3"));
  seeded.go(TaskList::new(TaskListTab::All).with_search("zzzz"));
  assert!(seeded.shows("tasks-no-rows"));
  seeded.go(TaskList::new(TaskListTab::All));
  seeded.until("the rows back at the top", |seeded| seeded.in_view("KON-3"));
}

#[test]
fn tasks_a_long_phrase_is_cut_with_an_ellipsis_inside_its_cell() {
  let mut seeded = Seeded::tasks("task-states", TaskList::new(TaskListTab::All));
  for key in seeded.keys() {
    let said = seeded.bounds(&format!("task-state-{key}"));
    assert!(said.width <= 320.5, "{key}: the phrase keeps to its 320: {said:?}");
  }
}

#[test]
fn tasks_done_lists_the_task_that_changed_last_first() {
  let mut seeded = Seeded::tasks("tasks", TaskList::new(TaskListTab::Done));
  let tasks = seeded.shown_tasks();
  let mut done: Vec<_> = tasks
    .tasks
    .iter()
    .filter(|task| task.organization().status().is_closed())
    .map(|task| {
      let changed = tasks.updated.get(&task.id()).copied().unwrap_or(task.created_at());
      (std::cmp::Reverse(changed), task.key().to_string())
    })
    .collect();
  done.sort();
  let expected: Vec<String> = done.into_iter().map(|(_, key)| key).collect();
  assert_eq!(seeded.keys(), expected);
  assert!(!seeded.shows("tasks-group-done"), "Done is a flat list");
}
