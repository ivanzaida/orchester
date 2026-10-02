//! Task list controls and scheduling review, driven through the headless desktop.
#[path = "task_bulk_recovery.rs"]
mod recovery;
use std::{path::Path, time::Duration};

use lurq::app::events::MouseButton;
use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  resource::{Pipeline, ResourceKind, Slug},
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
  review_readbacks(&mut desktop);
  desktop.tab_to("tasks-review-sequential", 30);
  desktop.enter();
  assert!(shows(&mut desktop, "tasks-review-parallel", "Run ready tasks together"));
  assert!(shows(&mut desktop, "tasks-review-summary", "2 tasks · one at a time"));
  desktop.tab_to("tasks-review-gate", 30);
  desktop.enter();
  desktop.enter();
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

fn review_readbacks(desktop: &mut Desktop) {
  review_geometry(desktop);
  assert!(shows(desktop, "tasks-review-summary", "2 tasks · up to 2 at once"));
  assert!(shows(desktop, "tasks-review-pipeline", "Delivery · v1"));
  assert!(shows(desktop, "tasks-review-cap-readback", "2.8M tokens"));
  assert!(shows(desktop, "tasks-review-budget-readback", "1.4M tokens"));
  assert!(shows(desktop, "tasks-review-time-readback", "30 min active agent time"));
  desktop.tab_to("tasks-review-failure", 30);
  desktop.enter();
  desktop.enter();
  desktop.tab_to("tasks-review-time-readback", 30);
  desktop.enter();
  desktop.tab_to("tasks-review-time", 30);
  desktop.key("End", "End");
  desktop.key("Backspace", "Backspace");
  desktop.key("Backspace", "Backspace");
  desktop.type_text("45");
  desktop.click("tasks-review-time-done");
  assert!(shows(desktop, "tasks-review-time-readback", "45 min active agent time"));
}

fn review_geometry(desktop: &mut Desktop) {
  let order = desktop.bounds("tasks-review-order");
  let rail = desktop.bounds("tasks-review-settings");
  let parallel = desktop.bounds("tasks-review-parallel");
  let sequential = desktop.bounds("tasks-review-sequential");
  assert!(
    (rail.x - order.x - order.width - 32.0).abs() < 1.0 && (parallel.x - rail.x).abs() < 1.0,
    "settings start 32 after the order's divider"
  );
  // The heading's line box differs by about a pixel between fonts; the gap after it is 18.
  assert!(
    (33.0..=36.0).contains(&(parallel.y - rail.y)),
    "the mode cards follow the heading 18 below"
  );
  assert!(
    (sequential.x + sequential.width - parallel.x - 508.0).abs() < 1.0,
    "controls must retain their full width beside the reserved gutter"
  );
  assert!((parallel.height - 88.0).abs() < 1.0 && (sequential.height - 88.0).abs() < 1.0);
  assert!((sequential.x - parallel.x - parallel.width - 12.0).abs() < 1.0);
}

#[test]
fn task_bulk_review_lists_mixed_published_pipeline_versions() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| {
    seed(repo, &fake, false);
    drafts(repo);
    let mut db = ProjectDb::open(repo).expect("db");
    let tasks = db.tasks().expect("tasks");
    let first = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
    let version = db
      .latest_version::<Pipeline>(first)
      .expect("version")
      .expect("published");
    let second = db
      .create_resource(
        Slug::new("alternate").expect("slug"),
        Name::new("Alternate").expect("name"),
        version.body(),
        Timestamp::now(),
      )
      .expect("pipeline");
    db.publish_draft::<Pipeline>(second.id(), Timestamp::now())
      .expect("publish");
    let mut details = tasks[1].details().clone();
    details.pipeline = Some(second.id());
    db.update_task(tasks[1].id(), details).expect("assign");
  });
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  let tasks = ProjectDb::open(desktop.repo()).expect("db").tasks().expect("tasks");
  for task in &tasks[..2] {
    desktop.wait_for(&format!("task-select-{}", task.key()));
    desktop.click(&format!("task-select-{}", task.key()));
  }
  let last = desktop.bounds(&format!("task-row-{}", tasks[2].key()));
  let footer = desktop.bounds("tasks-footer");
  assert!(footer.y >= last.y + last.height && footer.y < last.y + last.height + 24.0);
  assert!(shows(&mut desktop, "tasks-footer", "Open a task to edit"));
  desktop.click("run-selected");
  desktop.wait_for("tasks-review-pipeline");
  assert!(shows(
    &mut desktop,
    "tasks-review-pipeline",
    "Alternate · v1; Delivery · v1"
  ));
  desktop.click("tasks-review-cancel");
  assert!(shows(&mut desktop, "task-selection", "2 selected"));
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
  review_geometry(&mut desktop);
  keyboard_and_drag_reorder(&mut desktop);
  let reviewed_version = publish_unseen_pipeline(&mut desktop);
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
    assert_eq!(task.pipeline_version(), Some(reviewed_version));
    let run = db
      .run_with_key(task.id().first_run_key())
      .expect("first run")
      .expect("exists");
    assert_eq!(run.snapshot().pipeline().number, reviewed_version);
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

fn keyboard_and_drag_reorder(desktop: &mut Desktop) {
  let tasks = ProjectDb::open(desktop.repo()).expect("db").tasks().expect("tasks");
  let first = format!("tasks-review-order-{}", tasks[0].id());
  let second = format!("tasks-review-order-{}", tasks[1].id());
  desktop.tab_to("tasks-review-move-down", 40);
  desktop.enter();
  assert!(desktop.bounds(&first).y > desktop.bounds(&second).y);
  desktop.tab_to("tasks-review-order-undo", 40);
  desktop.enter();
  assert!(desktop.bounds(&first).y < desktop.bounds(&second).y);
  // Undo is consumed. Activating it again cannot change the restored order.
  desktop.click("tasks-review-order-undo");
  assert!(desktop.bounds(&first).y < desktop.bounds(&second).y);
  desktop.tab_to("tasks-review-order-task", 40);
  desktop.enter();
  desktop.key("ArrowDown", "ArrowDown");
  desktop.enter();
  desktop.tab_to("tasks-review-move-up", 40);
  desktop.enter();
  assert!(desktop.bounds(&first).y > desktop.bounds(&second).y);
  desktop.tab_to("tasks-review-order-undo", 40);
  desktop.enter();
  assert!(desktop.bounds(&first).y < desktop.bounds(&second).y);
  reorder(desktop);
  assert!(desktop.bounds(&first).y > desktop.bounds(&second).y);
  desktop.tab_to("tasks-review-order-undo", 40);
  desktop.enter();
  assert!(desktop.bounds(&first).y < desktop.bounds(&second).y);
  reorder(desktop);
  assert!(desktop.bounds(&first).y > desktop.bounds(&second).y);
}

#[test]
fn task_bulk_sequential_review_queues_once_and_runner_completes_each_task() {
  run_review("tasks-review-sequential");
}

#[test]
fn task_bulk_parallel_review_queues_once_and_runner_completes_each_task() {
  run_review("tasks-review-parallel");
}

#[test]
fn task_bulk_select_all_and_search_combine_with_drafts_tab() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-tasks");
  desktop.press("nav-tasks");
  desktop.wait_for("tasks-new-task");
  add_draft(&mut desktop, "Add search", 1);
  add_draft(&mut desktop, "Improve filters", 2);
  desktop.press("task-select-KON-1");
  assert!(shows(&mut desktop, "task-selection", "1 selected"));
  // Selecting another row keeps the task Create just made highlighted.
  assert!(super::new_task::highlighted(&mut desktop.tree, "KON-2"));
  desktop.press("tasks-select-all");
  assert!(shows(&mut desktop, "task-selection", "2 selected"));
  desktop.press("tasks-tab-drafts");
  desktop.wait_for("task-row-KON-1");
  desktop.press("tasks-search");
  desktop.type_text("filters");
  desktop.settle("the task search", |tree| {
    tree.get_element_by_id("task-row-KON-1").is_none() && tree.get_element_by_id("task-row-KON-2").is_some()
  });
  assert!(shows(&mut desktop, "task-selection", "2 selected"));
}

fn add_draft(desktop: &mut Desktop, title: &str, number: usize) {
  desktop.press("tasks-new-task");
  desktop.wait_for("task-title");
  desktop.tab_to("task-title", 30);
  desktop.type_text(title);
  // Enter is Create: the draft is made and the list shows it highlighted.
  desktop.enter();
  let key = format!("KON-{number}");
  desktop.settle("the new draft in the list", |tree| {
    tree.get_element_by_id("task-title").is_none() && super::new_task::highlighted(tree, &key)
  });
}

fn publish_unseen_pipeline(desktop: &mut Desktop) -> orchester_core::resource::VersionNumber {
  let mut db = ProjectDb::open(desktop.repo()).expect("db");
  let pipeline = db.resources(ResourceKind::Pipeline).expect("resources")[0].id();
  let version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("version")
    .expect("published");
  assert!(shows(desktop, "tasks-review-pipeline", &version.number().to_string()));
  let mut steps = version.body().steps().to_vec();
  steps[0].name = Name::new("Unseen step configuration").expect("name");
  let mut draft = version.to_draft(Timestamp::now());
  draft.edit(Pipeline::new(steps).expect("pipeline"), Timestamp::now());
  db.save_draft(pipeline, &draft).expect("draft");
  let newer = db
    .publish_draft::<Pipeline>(pipeline, Timestamp::now())
    .expect("publish");
  assert_ne!(newer.number(), version.number());
  assert!(shows(desktop, "tasks-review-pipeline", &version.number().to_string()));
  version.number()
}
