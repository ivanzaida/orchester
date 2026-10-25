//! *Stop turn* (ORC-92) end to end against `orchester-fake-claude` and `orchester-fake-codex`: a person stops the turn
//! of the agent at work; its CLI ends, its work is committed, the conversation shows the stopped turn, and its step
//! goes on in the same session with the person's message as its next instruction; the run is never paused. Without a
//! message the step waits for the person's next one, which it then continues with. With no agent at work there is no
//! turn to stop. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test turn_stop turn_stop`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::{
    AgentProvider,
    stream::{AgentEvent, InvocationOutcome},
  },
  conversation::{Arrival, Delivery, Message, MessageBody, Recipient, Timing},
  id::{IdempotencyKey, RunId, TaskId},
  run::{Run, RunStatus, StepStatus, WaitReason},
  store::{ActivityStore, ConversationStore},
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{Setup, TestProject, project, reported, scheduler, wait_for, wait_for_turns};

fn with(provider: AgentProvider) -> TestProject {
  project(&Setup {
    provider,
    ..Setup::new("Keep working.")
  })
}

/// Stores a person's message to the run, for after its agent's turn, as the desktop does.
fn send(root: &Path, task: TaskId, run: RunId, text: &str) -> Message {
  let message = Message::from_person(
    task,
    Some(run),
    Recipient::Orchestrator,
    MessageBody::new(text).expect("a message"),
    Timestamp::now(),
  )
  .sent(Timing::AfterTurn)
  .expect("a person's message");
  ProjectDb::open(root)
    .expect("open")
    .send_message(IdempotencyKey::generate(), &message)
    .expect("stored")
}

/// Starts the run and waits for its agent's first turn.
async fn started(project: &TestProject) -> (Scheduler, mpsc::UnboundedReceiver<Event>, RunId) {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  wait_for_turns(root, &mut events, id, 1).await;
  (scheduler, events, id)
}

fn ended(run: &Run) -> bool {
  run.status().is_terminal()
}

/// Whether a step of the run waits for the person's message.
fn waits_for_a_message(run: &Run) -> bool {
  run.current_attempt().is_some_and(|attempt| {
    attempt
      .executions()
      .iter()
      .any(|execution| execution.status() == StepStatus::Waiting(WaitReason::Message))
  })
}

/// The outcome of each invocation of the run, in order.
fn outcomes(root: &Path, run: RunId) -> Vec<Option<InvocationOutcome>> {
  ProjectDb::open(root)
    .expect("open")
    .invocations(run)
    .expect("invocations")
    .into_iter()
    .map(|invocation| invocation.outcome)
    .collect()
}

fn stopped_turns(root: &Path, run: RunId) -> usize {
  ProjectDb::open(root)
    .expect("open")
    .activity(run, 0)
    .expect("activity")
    .iter()
    .filter(|entry| entry.event == AgentEvent::TurnStopped)
    .count()
}

/// Stops the agent's turn with a message waiting for it: the run goes on in the same session, which reads the message
/// as its next instruction, and the step passes with no resume: the run was never paused.
async fn stops_a_turn_and_goes_on_with_the_message(provider: AgentProvider) {
  let project = with(provider);
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  send(root, project.task, id, "Rename the dialog first.");
  scheduler.deliver_messages(id).await.expect("taken");
  scheduler.stop_turn(id).await.expect("the turn stops");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded, "the run went on");
  assert!(run.last_checkpoint().is_some(), "the work was committed");
  let outcomes = outcomes(root, id);
  assert_eq!(outcomes.len(), 2, "{outcomes:?}");
  assert_eq!(outcomes[0], Some(InvocationOutcome::TurnStopped));
  assert_eq!(stopped_turns(root, id), 1, "the conversation shows the stopped turn");
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert!(
    invocations[1].session.is_some() && invocations[1].session == invocations[0].session,
    "the session continued: {:?} then {:?}",
    invocations[0].session,
    invocations[1].session
  );
  let messages = db.messages(project.task).expect("messages");
  assert!(
    matches!(
      messages[0].delivery(),
      Some(Delivery::Acknowledged { arrival: Arrival::TurnEnd, invocation, .. }) if invocation == invocations[1].id
    ),
    "{:?}",
    messages[0].delivery()
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn turn_stop_claude_goes_on_with_the_message_in_the_same_session() {
  stops_a_turn_and_goes_on_with_the_message(AgentProvider::ClaudeCodeCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn turn_stop_codex_goes_on_with_the_message_in_the_same_session() {
  stops_a_turn_and_goes_on_with_the_message(AgentProvider::CodexCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn turn_stop_without_a_message_waits_for_the_next_one_and_tells_the_agent_why() {
  let project = with(AgentProvider::ClaudeCodeCli);
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  scheduler.stop_turn(id).await.expect("the turn stops");
  let waiting = wait_for(root, &mut events, id, waits_for_a_message).await;
  assert_eq!(waiting.status(), &RunStatus::Running, "the run is not paused");
  assert!(!waiting.pause_requested());
  send(root, project.task, id, "Now rename the dialog.");
  scheduler.deliver_messages(id).await.expect("taken");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let prompt = reported(&db, id, "prompt| ").join("\n");
  assert!(
    prompt.contains("The person stopped your turn before you finished it.")
      && prompt.contains("Now rename the dialog.")
      && prompt.contains("Their message is your next instruction"),
    "{prompt}"
  );
  assert_eq!(
    scheduler.stop_turn(id).await,
    Err(Rejection::NoTurnToStop),
    "an ended run has no turn to stop"
  );
}
