//! What each row says about its task, in the tone the design gives it, and where a row, New task and a
//! Create's highlight lead.

use orchester_app::{
  routes::{ProjectPage, Route, TaskList, TaskListTab, TaskTab, TaskView},
  ui::icon::Icon,
};

use crate::harness::Seeded;

/// A row of the seed's `tasks`: its key, what its readiness cell starts with and the icon before it, and
/// its run line. No runner serves the project, so a run recorded as running is interrupted.
const ROWS: [(&str, &str, Icon, &str); 12] = [
  ("KON-1", "Merged into", Icon::GitMerge, "Merged"),
  ("KON-3", "Needs ", Icon::PencilLine, "Draft"),
  ("KON-5", "Ready to queue", Icon::CirclePlay, "Draft"),
  ("KON-8", "Waiting for its run to start", Icon::Hourglass, "Queued"),
  ("KON-9", "Every step passed", Icon::CircleCheck, "Completed"),
  ("KON-10", "Waiting for you", Icon::Hand, "Needs input"),
  ("KON-11", "Run failed", Icon::CircleX, "Failed · 04 Run checks"),
  ("KON-16", "No checkpoint yet", Icon::CirclePause, "Interrupted"),
  ("KON-17", "Held · a task failed", Icon::CirclePause, "Queued"),
  ("KON-19", "Blocked · KON-18 unfinished", Icon::Ban, "Draft"),
  ("KON-23", "Raise the limit to continue", Icon::Gauge, "Stopped at limit"),
  ("KON-24", "Run #", Icon::CircleHelp, "Unreadable"),
];

#[test]
fn tasks_readiness_says_what_each_phase_needs_including_held_blocked_and_unreadable() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  for (key, readiness, mark, run) in ROWS {
    let said = seeded.text(&format!("task-readiness-{key}"));
    assert!(said.starts_with(readiness), "{key}: {said}");
    assert_eq!(
      seeded.glyph(&format!("task-readiness-icon-{key}")),
      mark.glyph(),
      "{key}: the mark of {said}"
    );
    assert_eq!(seeded.text(&format!("task-run-{key}-label")), run, "{key}");
  }
  assert!(
    seeded.text("task-readiness-KON-24").ends_with("could not be read"),
    "an unreadable run says so"
  );
  assert!(seeded.shows("task-blocked-KON-19"), "a blocked task carries the flag");
  assert!(!seeded.shows("task-blocked-KON-18"));
}

#[test]
fn tasks_a_row_shows_its_key_title_tags_pipeline_priority_and_workflow() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  assert_eq!(seeded.texts("task-key-KON-12"), ["KON-12"]);
  assert_eq!(
    seeded.texts("task-title-KON-12"),
    ["Add retry to webhook sender", "backend", "webhooks"],
    "the tags in the project's order"
  );
  assert!(seeded.text("task-pipeline-KON-12").starts_with("Feature delivery"));
  assert_eq!(seeded.text("task-priority-KON-12"), "High");
  assert_eq!(seeded.text("task-workflow-KON-12"), "In progress");
  assert!(!seeded.shows("task-new-KON-12"), "only a Create's task is new");
  assert_eq!(
    seeded.texts("tasks-table-header"),
    [
      "ID",
      "TASK / TAGS",
      "PIPELINE",
      "PRIORITY",
      "WORKFLOW / RUN",
      "DEPENDENCY / READINESS"
    ]
  );
  assert_eq!(seeded.text("tasks-footer"), "24 tasks · 13 runs started");
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
  let key = "KON-22".parse().expect("a task key");
  let mut seeded = Seeded::tasks_in("tasks", TaskList::new(TaskListTab::All).with_highlight(key), 900);
  seeded.until("the new task in view", |seeded| seeded.in_view("KON-22"));
  assert!(
    !seeded.in_view("KON-1"),
    "the list scrolled to it: KON-1 is above the fold"
  );
}

#[test]
fn tasks_a_new_tab_sort_search_or_pipeline_starts_the_list_at_its_top() {
  let mut seeded = Seeded::tasks_in("tasks", TaskList::default(), 900);
  seeded.scroll_down();
  assert!(!seeded.in_view("KON-1"), "the list scrolled down");
  choose_sort(&mut seeded, "Newest first");
  seeded.until("a new sort to start at the top", |seeded| seeded.in_view("KON-24"));
  seeded.scroll_down();
  assert!(!seeded.in_view("KON-24"));
  seeded.go(TaskList::new(TaskListTab::Todo));
  seeded.until("a new tab to start at the top", |seeded| {
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
  assert!(seeded.in_view("KON-1"), "the rows wait at their top");
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
  let mut seeded = Seeded::tasks_in("tasks", TaskList::default(), 900);
  seeded.scroll_down();
  assert!(!seeded.in_view("KON-1"));
  seeded.go(TaskList::default().with_search("zzzz"));
  assert!(seeded.shows("tasks-no-rows"));
  seeded.go(TaskList::default());
  seeded.until("the rows back at the top", |seeded| seeded.in_view("KON-1"));
}

#[test]
fn tasks_a_long_readiness_is_cut_with_an_ellipsis_inside_its_cell() {
  let mut seeded = Seeded::tasks("task-states", TaskList::default());
  let said = seeded.bounds("task-readiness-KON-6");
  let row = seeded.bounds("task-row-KON-6");
  // The row ends in its 12 padding, the 14 chevron and the 12 before it.
  assert!(
    said.x + said.width <= row.x + row.width - 38.0 + 0.5,
    "the text stays in its cell: {said:?} in {row:?}"
  );
}

/// Chooses the sort whose label starts with `label`.
fn choose_sort(seeded: &mut Seeded, label: &str) {
  let mut select = seeded
    .window
    .tree
    .get_element_by_id_mut("tasks-sort")
    .and_then(|element| element.as_select())
    .expect("the sort is a select");
  let index = select
    .labels()
    .iter()
    .position(|option| option.starts_with(label))
    .expect("the sort offers it");
  select.commit(index);
  seeded.window.pass();
}

#[test]
fn tasks_a_failed_run_says_at_which_step_and_a_pipeline_reads_with_its_version() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  let failed = seeded.text("task-run-KON-11-label");
  assert!(
    failed.starts_with("Failed · 0") && failed.ends_with(" Run checks"),
    "{failed}"
  );
  assert_eq!(
    seeded.text("task-run-KON-9-label"),
    "Completed",
    "a finished run names no step"
  );
  assert_eq!(seeded.text("task-pipeline-KON-12"), "Feature delivery v3");
  assert_eq!(seeded.text("task-pipeline-KON-4"), "No pipeline");
}
