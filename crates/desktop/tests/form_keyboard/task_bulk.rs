//! Task list controls and scheduling review, driven through the headless desktop.
use std::{path::Path, time::Duration};

use lurq::app::events::MouseButton;
use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  resource::{Pipeline, ResourceKind},
  run::RunStatus,
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  task_run::{init_repository, programs, seed},
};

fn drafts(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open");
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  assert!(db.latest_version::<Pipeline>(pipeline).expect("version").is_some());
  let details = |title, brief: &str| TaskDetails {
    title: Name::new(title).expect("title"),
    brief: brief.into(),
    acceptance_criteria: vec!["One line is appended.".into()],
    pipeline: Some(pipeline),
    limits: Some(
      TaskLimits::new(TokenBudget::new(1_400_000).expect("budget"), Duration::from_secs(1800)).expect("limits"),
    ),
  };
  let created = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![
        details("Zulu", "Append one line."),
        details("Alpha", "Append one line."),
        details("Incomplete", ""),
      ],
      Timestamp::now(),
    )
    .expect("drafts");
  let mut incomplete = created[2].details().clone();
  incomplete.pipeline = None;
  db.update_task(created[2].id(), incomplete).expect("remove pipeline");
}

#[test]
fn task_bulk_selection_search_pipeline_assignment_and_review_keyboard() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed(repo, &fake, false);
    drafts(repo);
  });
  let keys: Vec<_> = ProjectDb::open(desktop.repo())
    .expect("db")
    .tasks()
    .expect("tasks")
    .iter()
    .map(|task| task.key().to_string())
    .collect();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for(&format!("task-select-{}", keys[0]));
  desktop.click(&format!("task-select-{}", keys[0]));
  assert!(shows(&mut desktop, "task-selection", "1 selected"));
  desktop.click(&format!("task-select-{}", keys[2]));
  assert!(shows(&mut desktop, "task-selection", "1 ready to queue · 1 incomplete"));
  desktop.click("set-pipeline");
  desktop.wait_for("tasks-assignment-pipeline");
  desktop.tab_to("tasks-assignment-pipeline", 30);
  desktop.enter();
  desktop.key("ArrowDown", "ArrowDown");
  desktop.enter();
  let picker = desktop
    .tree
    .get_element_by_id_mut("tasks-assignment-pipeline")
    .and_then(|e| e.as_select())
    .expect("picker");
  assert!(picker.is_selected(1));
  assert!(!picker.is_open());
  desktop.click("tasks-apply-pipeline");
  desktop.wait_until("the assignment completes", |desktop| {
    shows(desktop, &format!("task-row-{}", keys[2]), "Delivery")
  });
  desktop.click("tasks-cancel-pipeline");
  desktop.click(&format!("task-select-{}", keys[2]));
  desktop.press("tasks-search");
  desktop.type_text("Alpha");
  assert!(
    desktop
      .tree
      .get_element_by_id(&format!("task-select-{}", keys[1]))
      .is_some()
  );
  assert!(
    desktop
      .tree
      .get_element_by_id(&format!("task-select-{}", keys[0]))
      .is_none()
  );
  desktop.click("tasks-select-all");
  assert!(shows(&mut desktop, "task-selection", "2 selected"));
  desktop.click("run-selected");
  desktop.wait_for("tasks-review-parallel");
  desktop.tab_to("tasks-review-sequential", 30);
  desktop.enter();
  assert!(shows(&mut desktop, "tasks-review-parallel", "Run ready tasks together"));
  desktop.tab_to("tasks-review-cap", 30);
  assert_eq!(desktop.value("tasks-review-cap"), "2,800,000");
  desktop.type_text("0");
  desktop.click("tasks-review-cancel");
  assert!(shows(&mut desktop, "task-selection", "2 selected"));
  assert!(
    ProjectDb::open(desktop.repo())
      .expect("db")
      .task_groups()
      .expect("groups")
      .is_empty()
  );
}

fn run_review(mode: &str) {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed(repo, &fake, false);
    drafts(repo);
  });
  let keys: Vec<_> = ProjectDb::open(desktop.repo())
    .expect("db")
    .tasks()
    .expect("tasks")
    .iter()
    .map(|task| task.key().to_string())
    .collect();
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for(&format!("task-select-{}", keys[0]));
  desktop.click(&format!("task-select-{}", keys[0]));
  desktop.click(&format!("task-select-{}", keys[1]));
  desktop.click("run-selected");
  desktop.wait_for("tasks-review-sequential");
  desktop.click(mode);
  reorder(&mut desktop);
  desktop.wait_until("the runner online", |desktop| {
    shows(desktop, "status-runner", "Runner online")
  });
  desktop.click("tasks-review-queue");
  desktop.wait_for("tasks-review-result");
  desktop.wait_until("the review queues both tasks", |desktop| {
    !ProjectDb::open(desktop.repo())
      .expect("db")
      .task_groups()
      .expect("groups")
      .is_empty()
  });
  desktop.wait_until("both group runs succeed", |desktop| {
    let db = ProjectDb::open(desktop.repo()).expect("db");
    let tasks = db.tasks().expect("tasks");
    tasks[..2].iter().all(|task| {
      db.run_with_key(task.id().first_run_key())
        .expect("run")
        .is_some_and(|run| run.status() == &RunStatus::Succeeded)
    })
  });
  desktop.click("tasks-review-queue");
  let db = ProjectDb::open(desktop.repo()).expect("db");
  for task in &db.tasks().expect("tasks")[..2] {
    assert_eq!(db.runs_of_task(task.id()).expect("runs").len(), 1);
  }
  assert_eq!(db.task_groups().expect("groups").len(), 1);
  let tasks = db.tasks().expect("tasks");
  assert_eq!(
    db.task_groups().expect("groups")[0].tasks(),
    &[tasks[1].id(), tasks[0].id()]
  );
}

fn reorder(desktop: &mut Desktop) {
  let tasks = ProjectDb::open(desktop.repo()).expect("db").tasks().expect("tasks");
  let first = desktop.bounds(&format!("tasks-review-order-{}", tasks[0].id()));
  let next = desktop.bounds(&format!("tasks-review-order-{}", tasks[1].id()));
  let x = first.x + first.width / 2.0;
  let y = first.y + first.height / 2.0;
  let end = next.y + next.height / 2.0;
  desktop.tree.mouse_down(x, y, MouseButton::Left);
  for step in 1..=10 {
    desktop.tree.mouse_move(x, y + (end - y) * step as f32 / 10.0);
    desktop.pass();
  }
  desktop.tree.mouse_up(x, end, MouseButton::Left);
  desktop.pass();
}

#[test]
fn task_bulk_sequential_review_queues_once_and_runner_completes_each_task() {
  run_review("tasks-review-sequential");
}

#[test]
fn task_bulk_parallel_review_queues_once_and_runner_completes_each_task() {
  run_review("tasks-review-parallel");
}
