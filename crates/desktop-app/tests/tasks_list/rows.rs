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
  ("KON-11", "Run failed", Icon::CircleX, "Failed"),
  ("KON-16", "No checkpoint yet", Icon::CirclePause, "Interrupted"),
  ("KON-17", "Held · a task failed", Icon::CirclePause, "Queued"),
  ("KON-19", "Blocked · KON-18 unfinished", Icon::Ban, "Draft"),
  ("KON-23", "Raise the limit to continue", Icon::Gauge, "Stopped at limit"),
  ("KON-24", "Run #", Icon::FileWarning, "Unreadable"),
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
fn tasks_a_row_opens_the_task_s_summary_and_new_task_opens_the_form() {
  let mut seeded = Seeded::tasks("tasks", TaskList::default());
  seeded.click("task-row-KON-9");
  let summary = ProjectPage::Task("KON-9".parse().expect("a task key"), TaskView::of(TaskTab::Summary));
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, summary)
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
