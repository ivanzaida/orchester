//! Approval gates end to end against `orchester-fake-codex` and `orchester-fake-claude`: a gated
//! step's result becomes a plan the run waits on without holding an agent slot; approving continues
//! the graph with the plan as the step's output, requesting changes resumes the step's CLI session
//! with the reviewer's note and proposes the next version; a waiting run cancels, pauses and stays
//! waiting across a new runner. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test plans plans`.

mod support;

use std::{num::NonZeroU32, path::Path};

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, RunId},
  resource::{InjectionEvent, StepKey},
  run::{Plan, PlanDecision, PlanVersion, ReviewNote, Run, RunStatus, StepStatus, WaitReason},
  store::{ActivityStore, RunStore},
};
use orchester_ipc::{AttachSession, Event, Rejection, SessionEnd};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{Setup, StepSpec, project, queue_another_task, reported, scheduler, wait_for, wait_for_idle};

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("key")
}

/// Plan (gated), then Implement.
fn gated(provider: AgentProvider) -> Setup {
  Setup {
    steps: vec![
      StepSpec::new("plan", "Plan", "Propose a plan.", &[]).gate(),
      StepSpec::new("implement", "Implement", "Follow the plan.", &["plan"]),
    ],
    provider,
    ..Setup::new("Say hello.")
  }
}

fn changes(text: &str) -> PlanDecision {
  PlanDecision::ChangesRequested {
    note: ReviewNote::new(text).expect("a note"),
  }
}

fn plans(root: &Path, run: RunId) -> Vec<Plan> {
  ProjectDb::open(root).expect("open").plans(run).expect("plans")
}

fn waiting(run: &Run) -> bool {
  run.waits_for_a_person()
}

/// The plan proposals the scheduler announced.
fn proposals(events: &mut mpsc::UnboundedReceiver<Event>) -> Vec<(String, PlanVersion)> {
  let mut found = Vec::new();
  while let Ok(event) = events.try_recv() {
    if let Event::PlanProposed { task_key, version, .. } = event {
      found.push((task_key, version));
    }
  }
  found
}

/// Starts the task's run and waits until its plan v1 waits for a decision.
async fn proposed(root: &Path, scheduler: &Scheduler, events: &mut mpsc::UnboundedReceiver<Event>) -> (RunId, Plan) {
  let task = ProjectDb::open(root).expect("open");
  let task = orchester_core::store::TaskStore::tasks(&task).expect("tasks")[0].id();
  let id = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts");
  wait_for(root, events, id, waiting).await;
  let plan = plans(root, id).pop().expect("a plan");
  (id, plan)
}

#[tokio::test(flavor = "multi_thread")]
async fn plans_a_gate_waits_without_a_slot_and_approval_continues_the_graph() {
  let project = project(&gated(AgentProvider::CodexCli));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let one = NonZeroU32::MIN;
  let fresh = AttachSession::Fresh {
    previous_end: SessionEnd::Clean,
  };
  scheduler.attach(fresh, one).await.expect("attached");
  let (id, plan) = proposed(root, &scheduler, &mut events).await;
  assert_eq!(plan.version, PlanVersion::FIRST);
  assert_eq!(plan.step, key("plan"));
  // The agent's structured result is the plan.
  assert_eq!(plan.text, "Finished Plan");
  assert_eq!(plan.review, None);
  let run = ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("run");
  let status = run.latest_execution(&key("plan")).map(|execution| execution.status());
  assert_eq!(status, Some(StepStatus::Waiting(WaitReason::Plan)));
  assert_eq!(proposals(&mut events), [("ORC-1".to_owned(), PlanVersion::FIRST)]);
  // The waiting run's task ends and its agent slot is free: another run starts in it.
  wait_for_idle(&scheduler).await;
  let other = scheduler
    .start_run(IdempotencyKey::generate(), queue_another_task(root))
    .await
    .expect("the only slot is free");
  wait_for(root, &mut events, other, waiting).await;
  wait_for_idle(&scheduler).await;

  scheduler
    .decide_plan(id, plan.execution, plan.version, PlanDecision::Approved)
    .await
    .expect("the decision is recorded");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(plans(root, id)[0].decision(), Some(&PlanDecision::Approved));
  // Implement read the approved plan as the output of the step it runs after.
  let db = ProjectDb::open(root).expect("open");
  let prompt = reported(&db, id, "prompt| ");
  assert!(prompt.iter().any(|line| line == "Finished Plan"), "{prompt:?}");
  // A decided plan takes no second decision.
  assert_eq!(
    scheduler
      .decide_plan(id, plan.execution, plan.version, PlanDecision::Approved)
      .await,
    Err(Rejection::PlanNotAwaited)
  );
}

async fn requested_changes_resume_the_session(provider: AgentProvider) {
  let project = project(&gated(provider));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let (id, first) = proposed(root, &scheduler, &mut events).await;
  wait_for_idle(&scheduler).await;
  scheduler
    .decide_plan(id, first.execution, first.version, changes("Keep email sign-in."))
    .await
    .expect("the decision is recorded");
  wait_for(root, &mut events, id, |run| waiting(run) && plans(root, id).len() == 2).await;
  let [first, second] = <[Plan; 2]>::try_from(plans(root, id)).expect("two plans");
  assert_eq!(first.decision(), Some(&changes("Keep email sign-in.")));
  assert_eq!((second.version.get(), second.execution), (2, first.execution));
  assert_eq!(second.review, None);
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 2);
  // The revision resumed the session the plan came from, told what should change.
  assert_eq!(invocations[1].session, invocations[0].session);
  let sources = db
    .invocation_prompt(invocations[1].id)
    .expect("read")
    .expect("a prompt");
  assert_eq!(sources.event, InjectionEvent::Resume);
  let prompt = reported(&db, id, "prompt| ");
  assert!(
    prompt.iter().any(|line| line == "What should change in plan v1:"),
    "{prompt:?}"
  );
  assert!(prompt.iter().any(|line| line == "Keep email sign-in."), "{prompt:?}");
  // v1 is decided; only v2 waits.
  assert_eq!(
    scheduler
      .decide_plan(id, first.execution, first.version, PlanDecision::Approved)
      .await,
    Err(Rejection::PlanNotAwaited)
  );
  wait_for_idle(&scheduler).await;
  scheduler
    .decide_plan(id, second.execution, second.version, PlanDecision::Approved)
    .await
    .expect("the decision is recorded");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}

#[tokio::test(flavor = "multi_thread")]
async fn plans_requested_changes_resume_the_codex_session_with_the_note() {
  requested_changes_resume_the_session(AgentProvider::CodexCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn plans_requested_changes_resume_the_claude_session_with_the_note() {
  requested_changes_resume_the_session(AgentProvider::ClaudeCodeCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn plans_a_waiting_run_cancels_and_takes_no_decision_after() {
  let project = project(&gated(AgentProvider::CodexCli));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let (id, plan) = proposed(root, &scheduler, &mut events).await;
  wait_for_idle(&scheduler).await;
  scheduler.cancel_run(id).await.expect("cancelled");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Cancelled);
  let status = run.latest_execution(&key("plan")).map(|execution| execution.status());
  assert_eq!(status, Some(StepStatus::Skipped));
  assert_eq!(
    scheduler
      .decide_plan(id, plan.execution, plan.version, PlanDecision::Approved)
      .await,
    Err(Rejection::RunNotRunning)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn plans_a_waiting_run_pauses_and_keeps_waiting_across_a_new_runner() {
  let project = project(&gated(AgentProvider::CodexCli));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let (id, plan) = proposed(root, &scheduler, &mut events).await;
  wait_for_idle(&scheduler).await;
  // The next session's runner finds it running with no agent at work: it still waits.
  let (next, mut next_events) = support::scheduler(root);
  let crashed = AttachSession::Fresh {
    previous_end: SessionEnd::Crashed,
  };
  let attached = next.attach(crashed, NonZeroU32::MIN).await.expect("attached");
  assert!(attached.interrupted.is_empty());
  let run = ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("run");
  assert!(waiting(&run));
  // With no agent to settle, a pause is immediate; resuming waits for the decision again.
  next.pause_run(id).await.expect("paused");
  let paused = wait_for(root, &mut next_events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  let status = paused
    .latest_execution(&key("plan"))
    .map(|execution| execution.status());
  assert_eq!(status, Some(StepStatus::Waiting(WaitReason::Plan)));
  assert_eq!(
    next
      .decide_plan(id, plan.execution, plan.version, PlanDecision::Approved)
      .await,
    Err(Rejection::RunNotRunning)
  );
  next.resume_run(id).await.expect("resumed");
  wait_for(root, &mut next_events, id, waiting).await;
  wait_for_idle(&next).await;
  next
    .decide_plan(id, plan.execution, plan.version, PlanDecision::Approved)
    .await
    .expect("the decision is recorded");
  let run = wait_for(root, &mut next_events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}
