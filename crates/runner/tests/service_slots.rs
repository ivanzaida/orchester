//! *Services at once* (`docs/architecture/agent-testing.md`, decision 3) end to end against `orchester-fake-codex`,
//! with `orchester-test-command` as the services (`support::services`): a start beyond the project's share or the
//! computer's limit waits, noted in its run's transcript with the slots it needs, and the starts that wait go on in the
//! order they began to; a stack waits for a slot per service; a slot is freed when its service stops, when its run ends
//! and while its turn waits for a person; and a test suite's own process takes a slot too. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test service_slots service_slots`.

mod support;

use std::{fs, num::NonZeroU32, path::Path};

use orchester_core::{
  connection::stream::RunnerProblem,
  id::{IdempotencyKey, RunId, TaskId},
  resource::{ReportFormat, Slug, SuiteNeeds},
  run::RunStatus,
  service::ReadyCheck,
  store::RunStore,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{
  StepSpec,
  qa::{approve, calls, noted},
  queue_another_task, scheduler,
  services::{
    accepts, allow_computer_services, answered, closed, publish, publish_one, publish_stack, service, service_project,
    share_services, started_port,
  },
  suites::{NODE_TAP, commit, suite},
  wait_for,
};

fn implement(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("implement", "Implement", instructions, &[])]
}

/// Publishes the services `names`, each a stand-in server ready once it listens on its port.
fn publish_listeners(root: &Path, names: &[&str]) {
  let arguments = ["listen".to_owned(), "{{port}}".to_owned()];
  let services: Vec<_> = names
    .iter()
    .map(|name| service(name, &arguments, ReadyCheck::PortOpen, 20))
    .collect();
  publish(root, &services);
}

/// A scheduler of the project at `root` that lets `agents` agents work at once.
async fn scheduler_of(root: &Path, agents: u32) -> (Scheduler, mpsc::UnboundedReceiver<Event>) {
  let (scheduler, events) = scheduler(root);
  scheduler
    .set_agent_slots(NonZeroU32::new(agents).expect("above zero"))
    .await
    .expect("agents at once");
  (scheduler, events)
}

/// Starts a run of `task` and approves its first request, the start of its services.
async fn start(scheduler: &Scheduler, root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, task: TaskId) -> RunId {
  let run = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("run");
  approve(scheduler, root, events, run).await;
  run
}

/// Whether the run answered a call of `tool` yet.
fn has_answered(root: &Path, run: RunId, tool: &str) -> bool {
  calls(root, run).iter().any(|call| call.starts_with(tool))
}

fn waiting(slots: u32, share: u32, limit: u32) -> RunnerProblem {
  RunnerProblem::WaitingForServiceSlots { slots, share, limit }
}

#[tokio::test(flavor = "multi_thread")]
async fn service_slots_starts_beyond_the_project_share_wait_and_go_on_in_order() {
  let project = service_project(implement("Start the service web. Hold the services."), Vec::new());
  let root = project.repo.path();
  publish_listeners(root, &["web"]);
  share_services(root, 1);
  let tasks = [project.task, queue_another_task(root), queue_another_task(root)];
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler_of(root, 3).await;

  let first = start(&scheduler, root, &mut events, tasks[0]).await;
  let first_port = started_port(&answered(root, &mut events, first, "start_service").await);
  let second = start(&scheduler, root, &mut events, tasks[1]).await;
  noted(root, &mut events, second, waiting(1, 1, 8)).await;
  let third = start(&scheduler, root, &mut events, tasks[2]).await;
  noted(root, &mut events, third, waiting(1, 1, 8)).await;
  assert!(!has_answered(root, second, "start_service"), "the second start waits");
  assert!(!has_answered(root, third, "start_service"), "the third start waits");

  // The first run ends: its service stops and the earliest waiting start takes its slot.
  scheduler.cancel_run(first).await.expect("cancelled");
  let second_answer = answered(root, &mut events, second, "start_service").await;
  let second_port = started_port(&second_answer);
  assert!(second_answer.contains("waited_for_slots"), "{second_answer}");
  closed(first_port, "the first run's service once its run ended").await;
  assert!(accepts(second_port), "the second run's service runs");
  assert!(
    !has_answered(root, third, "start_service"),
    "the third start still waits: the second holds the project's only slot"
  );

  scheduler.cancel_run(second).await.expect("cancelled");
  let third_answer = answered(root, &mut events, third, "start_service").await;
  started_port(&third_answer);
  assert!(third_answer.contains("waited_for_slots"), "{third_answer}");
  fs::remove_file(&hold).expect("release");
  let ended = wait_for(root, &mut events, third, |run| run.status().is_terminal()).await;
  assert_eq!(ended.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn service_slots_a_stack_beyond_the_computers_limit_waits_for_a_slot_per_service() {
  let project = service_project(implement("Start the stack full. Hold the services."), Vec::new());
  let root = project.repo.path();
  publish_listeners(root, &["api", "web"]);
  publish_stack(root, "full", &["api", "web"]);
  allow_computer_services(root, 3);
  let second_task = queue_another_task(root);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler_of(root, 2).await;

  let first = start(&scheduler, root, &mut events, project.task).await;
  let first_answer = answered(root, &mut events, first, "start_stack").await;
  assert!(first_answer.starts_with("start_stack ok: "), "{first_answer}");
  // One of the computer's three slots is free, the project's share is the default 4: the stack needs two.
  let second = start(&scheduler, root, &mut events, second_task).await;
  noted(root, &mut events, second, waiting(2, 4, 3)).await;
  assert!(
    !has_answered(root, second, "start_stack"),
    "the stack waits for both slots"
  );

  scheduler.cancel_run(first).await.expect("cancelled");
  let second_answer = answered(root, &mut events, second, "start_stack").await;
  assert!(second_answer.starts_with("start_stack ok: "), "{second_answer}");
  assert!(second_answer.contains("waited_for_slots"), "{second_answer}");
  fs::remove_file(&hold).expect("release");
  let ended = wait_for(root, &mut events, second, |run| run.status().is_terminal()).await;
  assert_eq!(ended.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn service_slots_a_start_that_can_never_get_its_slots_is_refused_at_once() {
  let project = service_project(implement("Start the stack full."), Vec::new());
  let root = project.repo.path();
  publish_listeners(root, &["api", "web"]);
  publish_stack(root, "full", &["api", "web"]);
  allow_computer_services(root, 1);
  let (scheduler, mut events) = scheduler_of(root, 1).await;
  let run = start(&scheduler, root, &mut events, project.task).await;
  let answer = answered(root, &mut events, run, "start_stack").await;
  assert!(answer.starts_with("start_stack error: "), "{answer}");
  assert!(answer.contains("needs 2 service slots"), "{answer}");
  let ended = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
  assert_eq!(ended.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn service_slots_stop_service_frees_its_slot_for_another_run() {
  let project = service_project(
    implement("Start the service web. Then stop the service. Hold the services."),
    Vec::new(),
  );
  let root = project.repo.path();
  publish_listeners(root, &["web"]);
  share_services(root, 1);
  let second_task = queue_another_task(root);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler_of(root, 2).await;

  let first = start(&scheduler, root, &mut events, project.task).await;
  let stopped = answered(root, &mut events, first, "stop_service").await;
  assert!(stopped.contains("its service slot are free"), "{stopped}");
  // The first run still works, held, but its service stopped: the project's only slot is free.
  let second = start(&scheduler, root, &mut events, second_task).await;
  started_port(&answered(root, &mut events, second, "start_service").await);
  assert!(hold.exists(), "the first run is still at work");

  fs::remove_file(&hold).expect("release");
  for run in [first, second] {
    let ended = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
    assert_eq!(ended.status(), &RunStatus::Succeeded);
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn service_slots_are_freed_while_a_turn_waits_for_a_person() {
  let project = service_project(implement("Start the service web. Then read outside."), Vec::new());
  let root = project.repo.path();
  let deps = root.join("deps-a");
  fs::create_dir_all(&deps).expect("deps");
  fs::write(deps.join("notes.txt"), "outside line\n").expect("notes");
  publish_listeners(root, &["web"]);
  share_services(root, 1);
  let second_task = queue_another_task(root);
  let (scheduler, mut events) = scheduler_of(root, 2).await;

  let first = start(&scheduler, root, &mut events, project.task).await;
  let first_port = started_port(&answered(root, &mut events, first, "start_service").await);
  // The read outside the worktree asks: while the person decides, the first run's service stops and frees its slot.
  closed(first_port, "the first run's service while its turn waits for a person").await;
  let second = start(&scheduler, root, &mut events, second_task).await;
  started_port(&answered(root, &mut events, second, "start_service").await);
  let first_now = ProjectDb::open(root)
    .expect("open")
    .run(first)
    .expect("read")
    .expect("the run");
  assert!(
    !first_now.status().is_terminal(),
    "the second run took the slot while the first still waits for a person"
  );

  approve(&scheduler, root, &mut events, second).await;
  approve(&scheduler, root, &mut events, first).await;
  for run in [first, second] {
    let ended = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
    assert_eq!(ended.status(), &RunStatus::Succeeded);
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn service_slots_a_suites_own_process_takes_a_slot() {
  let project = service_project(implement("Run the suite tap."), Vec::new());
  let root = project.repo.path();
  commit(root, &[("fixtures/node.tap", NODE_TAP)]);
  publish_listeners(root, &["db"]);
  let tap = suite(&["cat", "fixtures/node.tap"], ReportFormat::Tap, None)
    .with_needs(SuiteNeeds::new(vec![Slug::new("db").expect("slug")]).expect("needs"));
  publish_one(root, "tap", &tap);
  // `db` takes the project's only slot; the suite's own process would need a second one.
  share_services(root, 1);
  let (scheduler, mut events) = scheduler_of(root, 1).await;
  let run = start(&scheduler, root, &mut events, project.task).await;
  let answer = answered(root, &mut events, run, "run_tests").await;
  assert!(answer.starts_with("run_tests error: "), "{answer}");
  assert!(answer.contains("holds 1 already"), "{answer}");
  let ended = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
  assert_eq!(ended.status(), &RunStatus::Succeeded);
}
