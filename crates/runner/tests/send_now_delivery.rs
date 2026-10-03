//! Send now (ORC-61) delivered exactly once, end to end against `orchester-fake-codex` and
//! `orchester-fake-claude`: a message on its way to one agent at work goes into no other agent's
//! prompt meanwhile; Codex's acceptance of a steer read after its turn completed still delivers the
//! message at the turn's end; and a message Codex accepted stays queued, for the step's next
//! invocation, when the turn is interrupted or the process dies before the turn completes, since
//! Codex then did not keep it.
//! Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test send_now_delivery send_now_`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  conversation::{Arrival, Delivery, Message, MessageBody, Recipient, Timing},
  id::{IdempotencyKey, RunId, StepExecutionId, TaskId},
  resource::StepKey,
  run::{Run, RunStatus},
  store::{ActivityStore, ConversationStore},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{
  PATIENCE, Setup, StepSpec, TestProject, project, reported, scheduler, tools::ToolSpec, wait_for, wait_for_turns,
};

const TEXT: &str = "Rename the test.";

fn with(provider: AgentProvider, brief: &'static str) -> TestProject {
  project(&Setup {
    provider,
    ..Setup::new(brief)
  })
}

/// Stores a person's message to the run, sent now, as the desktop does.
fn send_now(root: &Path, task: TaskId, run: RunId) {
  let message = Message::from_person(
    task,
    Some(run),
    Recipient::Orchestrator,
    MessageBody::new(TEXT).expect("a message"),
    Timestamp::now(),
  )
  .sent(Timing::Now)
  .expect("a person's message");
  ProjectDb::open(root)
    .expect("open")
    .send_message(IdempotencyKey::generate(), &message)
    .expect("stored");
}

fn delivery(root: &Path, task: TaskId) -> Option<Delivery> {
  let messages = ProjectDb::open(root).expect("open").messages(task).expect("messages");
  messages.iter().find_map(Message::delivery)
}

fn release(root: &Path) {
  std::fs::write(root.join(".orchester").join("fake-release"), "go").expect("release");
}

fn ended(run: &Run) -> bool {
  run.status().is_terminal()
}

/// The agent turns that started in the run.
fn turns(root: &Path, run: RunId) -> usize {
  ProjectDb::open(root)
    .expect("open")
    .activity(run, 0)
    .expect("activity")
    .iter()
    .filter(|entry| entry.event == AgentEvent::TurnStarted)
    .count()
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

fn execution_of(run: &Run, step: &str) -> StepExecutionId {
  run
    .latest_execution(&StepKey::new(step).expect("key"))
    .expect("the step ran")
    .id()
}

#[tokio::test(flavor = "multi_thread")]
async fn send_now_a_message_on_its_way_to_one_agent_goes_to_no_other() {
  // Implement works until released; Review starts beside it once the tool step before it ends,
  // while Implement's CLI has the message but has not taken it yet.
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    steps: vec![
      StepSpec::new("implement", "Implement", "Finish after release.", &[]),
      StepSpec::new("pause", "Pause", "", &[]).command(ToolSpec::test_command(&["sleep", "4"])),
      StepSpec::new("review", "Review", "Say hello.", &["pause"]),
    ],
    ..Setup::new("Work on the greeting.")
  });
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  send_now(root, project.task, id);
  scheduler.deliver_messages(id).await.expect("taken");
  let written = async {
    while !reported(&ProjectDb::open(root).expect("open"), id, "input: ").contains(&TEXT.to_owned()) {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, written).await.expect("written in time");
  assert_eq!(turns(root, id), 1, "Review has not started yet");
  wait_for_turns(root, &mut events, id, 2).await;
  release(root);
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let implement = execution_of(&run, "implement");
  assert!(
    matches!(
      delivery(root, project.task),
      Some(Delivery::Delivered { execution, .. } | Delivery::Acknowledged { execution, .. }) if execution == implement
    ),
    "{:?}",
    delivery(root, project.task)
  );
  let db = ProjectDb::open(root).expect("open");
  assert!(
    !reported(&db, id, "prompt| ").iter().any(|line| line.contains(TEXT)),
    "no agent's prompt took it"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn send_now_codex_acceptance_read_after_the_turn_completed_delivers_once() {
  let project = with(AgentProvider::CodexCli, "Answer the steer late.");
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  send_now(root, project.task, id);
  scheduler.deliver_messages(id).await.expect("taken");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(turns(root, id), 1, "not sent again in a turn of its own");
  assert!(
    matches!(
      delivery(root, project.task),
      Some(
        Delivery::Delivered {
          arrival: Arrival::TurnEnd,
          ..
        } | Delivery::Acknowledged {
          arrival: Arrival::TurnEnd,
          ..
        }
      )
    ),
    "{:?}",
    delivery(root, project.task)
  );
}

/// Codex accepts the message, then does not keep it as `brief` says: the turn's invocation does not
/// deliver it, so it stays queued for the step's next one, which restarts the step broken this way.
async fn accepted_but_not_kept(brief: &'static str) {
  let project = with(AgentProvider::CodexCli, brief);
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  send_now(root, project.task, id);
  scheduler.deliver_messages(id).await.expect("taken");
  let task = project.task;
  wait_for(root, &mut events, id, |run| {
    ended(run) || delivery(root, task) != Some(Delivery::Queued)
  })
  .await;
  let first = ProjectDb::open(root)
    .expect("open")
    .invocations(id)
    .expect("invocations")[0]
    .id;
  match delivery(root, task) {
    Some(Delivery::Queued) => {}
    Some(Delivery::Delivered {
      invocation, arrival, ..
    }) => {
      assert_eq!(arrival, Arrival::Prompt, "with the next invocation's prompt");
      assert_ne!(invocation, first, "not to the invocation that did not keep it");
    }
    other => panic!("{other:?}"),
  }
  scheduler.cancel_run(id).await.expect("cancelled");
  wait_for(root, &mut events, id, ended).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn send_now_codex_accepted_stays_queued_when_the_turn_is_interrupted() {
  accepted_but_not_kept("Interrupt after a message.").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn send_now_codex_accepted_stays_queued_when_the_process_dies() {
  accepted_but_not_kept("Die after a message.").await;
}
