//! A blocked task in the task list: Run selected names it as a problem and does not queue it, its
//! row and page say what blocks it, and bulk Canceled asks before stopping a run that waits for a
//! person.

use std::{path::Path, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  name::Name,
  resource::ResourceKind,
  run::StepFailure,
  store::{ResourceStore, RunStore, TaskStore},
  task::{
    TaskDetails, TaskLimits,
    organization::{BlockingTask, ChangeActor, DependencyGate, Priority, TaskOrganization, WorkflowStatus},
  },
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;

use crate::{
  github::shows,
  harness::Desktop,
  unreadable_run::{at, delivery, started},
};

fn complete(pipeline: ResourceId, title: &str) -> TaskDetails {
  TaskDetails {
    title: Name::new(title).expect("title"),
    brief: "Implement.".into(),
    acceptance_criteria: vec!["Complete.".into()],
    pipeline: Some(pipeline),
    limits: Some(
      TaskLimits::new(TokenBudget::new(10_000).expect("budget"), Duration::from_secs(1_800)).expect("limits"),
    ),
  }
}

/// KON-1 and KON-2, complete drafts; KON-2 waits for KON-1 to be ready for review.
fn seed_blocked(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open");
  delivery(&mut db);
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let tasks = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![complete(pipeline, "Blocking"), complete(pipeline, "Blocked")],
      at(10),
    )
    .expect("tasks");
  let linked = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::None,
    Some("Needs the first task.".into()),
    vec![BlockingTask {
      task: tasks[0].id(),
      gate: DependencyGate::Review,
    }],
    vec![],
  )
  .expect("organization");
  db.set_task_organization(tasks[1].id(), &linked, ChangeActor::Person, at(11))
    .expect("link");
}

fn reachable(desktop: &mut Desktop, id: &str) -> bool {
  desktop.stops(60, false).contains(&Some(id.into()))
}

#[test]
fn organization_blocked_draft_is_a_selection_problem_and_says_what_blocks_it() {
  let mut desktop = Desktop::with_seeded_project(seed_blocked);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-2");
  assert!(shows(&mut desktop, "task-row-KON-2", "KON-1 unfinished"));
  desktop.press("task-select-KON-1");
  desktop.wait_until("Run selected for the ready draft", |desktop| {
    reachable(desktop, "run-selected")
  });
  desktop.press("task-select-KON-2");
  desktop.wait_for("task-problems");
  assert!(shows(&mut desktop, "task-problems", "KON-2 is blocked by KON-1"));
  assert!(!reachable(&mut desktop, "run-selected"), "a blocked task is not queued");
  desktop.press("task-row-KON-2");
  desktop.wait_for("task-blocked-run-unavailable");
  assert!(shows(
    &mut desktop,
    "task-blocked-run-unavailable",
    "Run is unavailable while KON-1 is unfinished."
  ));
}

/// KON-1 with run #1 waiting for a person after its step broke again.
fn seed_stuck(repo: &Path) {
  stuck_task(repo);
}

/// [`seed_stuck`], with KON-1 linked, without a reason, to KON-2, a draft that has not run.
fn seed_stuck_and_blocked(repo: &Path) {
  let task = stuck_task(repo);
  let mut db = ProjectDb::open(repo).expect("open");
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let blocker = db
    .create_tasks(IdempotencyKey::generate(), vec![complete(pipeline, "Blocking")], at(40))
    .expect("blocker")[0]
    .id();
  let linked = TaskOrganization::new(
    WorkflowStatus::InProgress,
    Priority::None,
    None,
    vec![BlockingTask {
      task: blocker,
      gate: DependencyGate::Review,
    }],
    vec![],
  )
  .expect("organization");
  db.set_task_organization(task, &linked, ChangeActor::Person, at(41))
    .expect("link");
}

fn stuck_task(repo: &Path) -> orchester_core::id::TaskId {
  let mut db = ProjectDb::open(repo).expect("open");
  let snapshot = delivery(&mut db);
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let task = db
    .create_tasks(IdempotencyKey::generate(), vec![complete(pipeline, "Stuck")], at(10))
    .expect("task")[0]
    .id();
  let mut run = started(&mut db, (task, &snapshot), 1, 11);
  for millis in [20, 30] {
    let execution = run.current_attempt().expect("attempt").executions()[0].id();
    let events = run.start_step(execution, at(millis)).expect("step starts");
    db.append_events(&run, &events).expect("stored");
    let events = run
      .fail_step(execution, StepFailure::AgentFailed, at(millis + 1))
      .expect("step fails");
    db.append_events(&run, &events).expect("stored");
  }
  assert!(matches!(
    run.status(),
    orchester_core::run::RunStatus::WaitingForPerson(_)
  ));
  task
}

/// A run that waits for a person has not ended: bulk Canceled asks to stop it.
#[test]
fn organization_bulk_canceled_asks_for_a_run_that_waits_for_a_person() {
  let mut desktop = Desktop::with_seeded_project(seed_stuck);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.press("task-select-KON-1");
  desktop.press("bulk-status");
  desktop.wait_for("bulk-status-Canceled");
  desktop.press("bulk-status-Canceled");
  desktop.wait_for("bulk-cancel-stop");
}

/// A run that waits for a person's decision on a task blocked by a link alone: the decisions that
/// let it go on are unavailable, the notice names the blocking task, and the header says Blocked.
#[test]
fn organization_waiting_run_of_a_blocked_task_says_why_it_cannot_go_on() {
  let mut desktop = Desktop::with_seeded_project(seed_stuck_and_blocked);
  desktop.wait_for("nav-tasks");
  desktop.click("nav-tasks");
  desktop.wait_for("task-row-KON-1");
  desktop.press("task-row-KON-1");
  desktop.wait_for("task-blocked-run-unavailable");
  assert!(shows(
    &mut desktop,
    "task-blocked-run-unavailable",
    "Run is unavailable while KON-2 is unfinished."
  ));
  assert!(desktop.tree.get_element_by_id("task-header-blocked").is_some());
  assert!(
    !reachable(&mut desktop, "task-allow-retry"),
    "another attempt is unavailable"
  );
}
