//! ORC-25 run #23 end to end against `orchester-fake-codex` (the project task ORC-44 · Return a finished run to an
//! earlier step): a run of Implement → Check → Review → Verify succeeded, the person asks Verify again, and its
//! follow-up reports the work failed. The run leaves Succeeded and goes back to Implement by Verify's failure route,
//! with Verify's report in Implement's prompt, then runs Check, Review and Verify again and is ready for review again.
//! Where Verify has no route, the run waits for the person instead. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test follow_up_routing follow_up_routing`.

mod support;

use std::{path::Path, time::Duration};

use orchester_core::{
  conversation::{Message, MessageBody, Recipient},
  id::{IdempotencyKey, RunId, TaskId},
  resource::StepKey,
  run::{AttemptReason, AttemptStatus, Run, RunStatus, StepFailure, StepStatus, Stuck},
  store::{ConversationStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{Setup, StepSpec, project, reported, scheduler, tools::ToolSpec, wait_for, wait_for_idle};

/// How long a follow-up and the attempts it is routed to may take.
const ROUTED_PATIENCE: Duration = Duration::from_secs(120);

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("key")
}

/// Implement → Check → Review → Verify, Verify's failure returning to `route` when given.
fn pipeline(route: Option<&'static str>) -> Setup {
  let verify = StepSpec::new("verify", "Verify", "Verify the change. Leave a handoff.", &["review"]);
  Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Change README.md.", &[]),
      StepSpec::new("check", "Check", "", &["implement"]).command(ToolSpec::test_command(&["print", "1"])),
      StepSpec::new("review", "Review", "Review the change.", &["check"]),
      match route {
        Some(to) => verify.retry(to, 2),
        None => verify,
      },
    ],
    ..Setup::new("Say hello.")
  }
}

/// Stores a person's message to the task's run, as the desktop does: to the Orchestrator, so it reaches the last agent
/// step, Verify. The fake agent's resumed turn reports the work failed, with a handoff, as the message asks.
fn ask_verify_again(root: &Path, task: TaskId, run: RunId) {
  let message = Message::from_person(
    task,
    Some(run),
    Recipient::Orchestrator,
    MessageBody::new("The empty body still panics. Answer failed. Leave a handoff.").expect("a message"),
    Timestamp::now(),
  );
  ProjectDb::open(root)
    .expect("open")
    .send_message(IdempotencyKey::generate(), &message)
    .expect("stored");
}

/// The run of `setup` once it succeeded and no task executes it, with its scheduler and events.
async fn ready(setup: &Setup) -> (support::TestProject, Scheduler, mpsc::UnboundedReceiver<Event>, Run) {
  let project = project(setup);
  let (scheduler, mut events) = scheduler(project.repo.path());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(project.repo.path(), &mut events, id, support::settled).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(run.status(), &RunStatus::Succeeded, "{:?}", run.attempts());
  (project, scheduler, events, run)
}

/// The run once it settled again after its first `first` attempts: the follow-up and whatever it was routed to, which
/// may be a whole pass of fresh sessions, longer than [`support::PATIENCE`] gives one wait.
async fn settled_after(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, id: RunId, first: usize) -> Run {
  let wait = async {
    loop {
      let current = ProjectDb::open(root)
        .expect("open")
        .run(id)
        .expect("read")
        .expect("run exists");
      if support::settled(&current) && current.attempts().len() > first {
        return current;
      }
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(ROUTED_PATIENCE, wait)
    .await
    .expect("the run settles again in time")
}

fn steps(run: &Run, attempt: usize) -> Vec<(String, StepStatus)> {
  run.attempts()[attempt]
    .executions()
    .iter()
    .map(|execution| (execution.step().as_str().to_owned(), execution.status()))
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn follow_up_routing_a_failed_follow_up_returns_the_run_to_implement_and_runs_the_steps_after_it() {
  let (project, scheduler, mut events, ready) = ready(&pipeline(Some("implement"))).await;
  let root = project.repo.path();
  let id = ready.id();
  let first = ready.attempts().len();
  ask_verify_again(root, project.task, id);
  scheduler.deliver_messages(id).await.expect("followed up");
  let run = settled_after(root, &mut events, id, first).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(run.status(), &RunStatus::Succeeded, "{:?}", run.attempts());
  assert_eq!(run.attempts().len(), first + 2, "{:?}", run.attempts());

  let follow_up = &run.attempts()[first];
  assert_eq!(follow_up.reason(), &AttemptReason::FollowUp { step: key("verify") });
  assert_eq!(
    follow_up.status(),
    AttemptStatus::RoutedBack,
    "the follow-up did not leave the run succeeded"
  );
  assert_eq!(follow_up.executions()[0].failure(), Some(StepFailure::ReportedFailed));
  let retry = &run.attempts()[first + 1];
  assert_eq!(
    retry.reason(),
    &AttemptReason::Retry {
      failed_step: key("verify"),
      return_to: key("implement"),
    }
  );
  assert_eq!(
    steps(&run, first + 1),
    ["implement", "check", "review", "verify"].map(|step| (step.to_owned(), StepStatus::Passed))
  );

  // Implement read why the work came back: the follow-up's report, its handoff quoted.
  let db = ProjectDb::open(root).expect("open");
  let prompts = reported(&db, id, "prompt| ");
  let returned = format!("# Returned from Verify (`verify`), attempt {}", follow_up.number());
  for line in [
    returned.as_str(),
    "## What Verify reported",
    "> The next agent should know:",
    "> - README.md ends with hello",
  ] {
    assert!(prompts.iter().any(|prompt| prompt == line), "{line} in {prompts:?}");
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn follow_up_routing_a_failed_follow_up_without_a_route_waits_for_the_person() {
  let (project, scheduler, mut events, ready) = ready(&pipeline(None)).await;
  let root = project.repo.path();
  let id = ready.id();
  ask_verify_again(root, project.task, id);
  scheduler.deliver_messages(id).await.expect("followed up");
  let run = settled_after(root, &mut events, id, ready.attempts().len()).await;
  wait_for_idle(&scheduler).await;
  assert_eq!(
    run.status(),
    &RunStatus::WaitingForPerson(Stuck::RetriesExhausted(key("verify")))
  );
  assert_eq!(run.return_targets(), ["implement", "review", "verify"].map(key));
  assert!(scheduler.active_runs().is_empty(), "a waiting run holds no agent slot");
}
