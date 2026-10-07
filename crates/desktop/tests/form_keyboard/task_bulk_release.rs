//! A group held for good keeps its unstarted tasks queued until the person takes them back to
//! the drafts from the selection bar.
use std::path::Path;

use orchester_core::{
  id::IdempotencyKey,
  resource::Pipeline,
  store::{ResourceStore, TaskStore},
  task::TaskStatus,
  task_group::{GroupHold, ReviewedPipeline, Scheduling, TaskGroup},
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
  task_run::{programs, seed},
};

/// Zulu and Alpha queued in a group a failed task holds; Incomplete stays a draft.
pub(crate) fn held_group(repo: &Path) {
  let (_, fake) = programs();
  seed(repo, &fake, false);
  super::drafts(repo);
  let mut db = ProjectDb::open(repo).expect("db");
  let tasks: Vec<_> = db.tasks().expect("tasks")[..2].iter().map(|task| task.id()).collect();
  let reviewed: Vec<_> = tasks
    .iter()
    .map(|id| {
      let pipeline = db
        .task(*id)
        .expect("task")
        .expect("exists")
        .details()
        .pipeline
        .expect("pipeline");
      let version = db
        .latest_version::<Pipeline>(pipeline)
        .expect("version")
        .expect("published")
        .number();
      ReviewedPipeline {
        task: *id,
        pipeline,
        version,
      }
    })
    .collect();
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    tasks,
    Scheduling::Sequential,
    TokenBudget::new(2_800_000).expect("cap"),
  )
  .expect("group");
  db.queue_task_group(&group, None, &reviewed).expect("queue");
  db.set_task_group_hold(group.key(), Some(&GroupHold::FailedTask))
    .expect("hold");
}

#[test]
fn task_bulk_release_returns_held_queued_tasks_to_the_drafts() {
  let mut desktop = Desktop::with_seeded_project(held_group);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-select-KON-2");
  assert!(shows(&mut desktop, "task-row-KON-1", "Held · a task failed"));
  // Selecting a task without one shows no release action.
  desktop.press("task-select-KON-3");
  assert!(desktop.tree.get_element_by_id("tasks-release-held").is_none());
  desktop.press("task-select-KON-1");
  desktop.press("task-select-KON-2");
  assert!(shows(
    &mut desktop,
    "task-selection",
    "0 ready to queue · 1 incomplete · 2 already queued or started"
  ));
  // The two held tasks share one short line (02.03D `IYlGK`).
  assert!(shows(
    &mut desktop,
    "task-problems",
    "2 tasks held after a task failed · Return to drafts"
  ));
  desktop.press("tasks-release-held");
  desktop.settle("the released drafts", |tree| {
    tree
      .get_element_by_id("task-selection")
      .is_some_and(|bar| contains_text(bar, "2 ready to queue · 1 incomplete"))
      && tree.get_element_by_id("tasks-release-held").is_none()
  });
  let db = ProjectDb::open(desktop.repo()).expect("db");
  for task in &db.tasks().expect("tasks")[..2] {
    assert_eq!(task.status(), TaskStatus::Draft);
    assert!(db.task_group_for_task(task.id()).expect("group").is_none());
  }
  assert!(db.task_groups().expect("groups").is_empty());
}

#[test]
fn task_bulk_release_actions_fit_the_narrowest_window() {
  let mut desktop = Desktop::with_seeded_project(held_group);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-select-KON-2");
  desktop.press("task-select-KON-1");
  desktop.press("task-select-KON-2");
  desktop.wait_for("tasks-release-held");
  let wide = desktop.bounds("set-pipeline").width;
  desktop.tree.resize(960, 960);
  desktop.settle("the narrow selection bar", |tree| {
    tree
      .get_element_by_id_mut("tasks-release-held")
      .and_then(|button| button.bounds())
      .is_some_and(|bounds| bounds.x + bounds.width <= 960.0)
  });
  let bar = desktop.bounds("task-selection");
  for id in ["tasks-release-held", "set-pipeline", "run-selected"] {
    let button = desktop.bounds(id);
    assert!(
      button.x >= bar.x && button.x + button.width <= bar.x + bar.width && button.x + button.width <= 960.0,
      "{id} is inside the bar and the window: {button:?}"
    );
  }
  assert!(
    (desktop.bounds("set-pipeline").width - wide).abs() < 0.5,
    "Set pipeline keeps its width"
  );
  assert!(shows(&mut desktop, "task-problems", "2 tasks held after a task failed"));
}

/// The selection bar is hidden while nothing is selected; once tasks are, its run actions end with
/// Run selected at the right of the selection's line, above the organization group (02.03D).
#[test]
fn task_bulk_bar_hidden_without_a_selection_and_run_selected_last_at_1440() {
  let mut desktop = Desktop::with_seeded_project(held_group);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-select-KON-2");
  assert!(desktop.tree.get_element_by_id("task-selection").is_none());
  desktop.press("task-select-KON-1");
  desktop.press("task-select-KON-2");
  desktop.wait_for("tasks-release-held");
  let (bar, select_all) = (desktop.bounds("task-selection"), desktop.bounds("tasks-select-all"));
  let (release, pipeline, run) = (
    desktop.bounds("tasks-release-held"),
    desktop.bounds("set-pipeline"),
    desktop.bounds("run-selected"),
  );
  assert!(
    release.x < pipeline.x && pipeline.x < run.x,
    "Return to drafts, Set pipeline, Run selected"
  );
  assert!(
    (bar.x + bar.width - 12.0 - (run.x + run.width)).abs() < 0.5,
    "Run selected ends 12 from the bar's edge"
  );
  let middle = |rect: &lurq::core::ElementRect| rect.y + rect.height / 2.0;
  assert!(
    (middle(&run) - middle(&select_all)).abs() < 1.0,
    "one line with the selection"
  );
  assert!(
    (run.y - bar.y - 12.0).abs() < 0.5 && (run.height - 32.0).abs() < 0.5,
    "padded 12, 32 high"
  );
  let status = desktop.bounds("bulk-status");
  assert!(
    (status.y - (run.y + run.height + 8.0)).abs() < 0.5,
    "the organization line 8 below"
  );
  desktop.press("task-select-KON-1");
  desktop.press("task-select-KON-2");
  desktop.settle("the hidden bar", |tree| {
    tree.get_element_by_id("task-selection").is_none()
  });
}

/// At 960 the run actions take a line of their own, right-aligned, between the selection and the
/// organization group, and nothing leaves the window (02.03E `t7ETG`).
#[test]
fn task_bulk_bar_three_lines_at_960() {
  let mut desktop = Desktop::with_seeded_project(held_group);
  desktop.tree.resize(960, 960);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-select-KON-2");
  desktop.press("task-select-KON-1");
  desktop.press("task-select-KON-2");
  desktop.wait_for("tasks-release-held");
  let (bar, select_all) = (desktop.bounds("task-selection"), desktop.bounds("tasks-select-all"));
  let (release, run, status) = (
    desktop.bounds("tasks-release-held"),
    desktop.bounds("run-selected"),
    desktop.bounds("bulk-status"),
  );
  assert!(
    run.y > select_all.y + select_all.height,
    "the run actions under the selection"
  );
  assert!(
    status.y > run.y + run.height,
    "the organization group under the run actions"
  );
  assert!(
    (bar.x + bar.width - 12.0 - (run.x + run.width)).abs() < 0.5,
    "right-aligned"
  );
  assert!(release.x < run.x && bar.x + bar.width <= 960.0);
}

/// Rows are 76 high with their tag chips at 1440 and 60 high at 960, where the chips are gone.
#[test]
fn task_bulk_rows_76_high_at_1440_and_60_at_960() {
  let mut desktop = Desktop::with_seeded_project(held_group);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  assert!((desktop.bounds("task-row-KON-1").height - 76.0).abs() < 0.5);
  desktop.tree.resize(960, 960);
  desktop.settle("the narrow table", |tree| {
    tree
      .get_element_by_id_mut("task-row-KON-1")
      .and_then(|row| row.bounds())
      .is_some_and(|row| (row.height - 60.0).abs() < 0.5)
  });
}
