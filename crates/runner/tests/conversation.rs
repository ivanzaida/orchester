//! The task conversation end to end against `orchester-fake-codex` and `orchester-fake-claude`: a
//! message to an agent at work reaches it once its turn is over, in the standard input of its resumed
//! session; a question makes its step wait without an agent slot and the answer resumes the session;
//! Ask agent after a run succeeded continues the last agent step's session and records a new
//! checkpoint; a run waiting for an answer cancels; a message written before the run reaches its first
//! step. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test conversation conversation`.

mod support;

use std::{path::Path, time::Duration};

use orchester_core::{
  connection::AgentProvider,
  conversation::{AgentMessage, Delivery, Message, MessageBody, Recipient, Sender},
  id::{IdempotencyKey, RunId, TaskId},
  resource::{InjectionEvent, StepKey},
  run::{AttemptReason, Run, RunStatus, StepStatus, WaitReason},
  store::{ActivityStore, ConversationStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::{AttachSession, Event, Rejection};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{
  PATIENCE, Setup, StepSpec, TestProject, git, project, reported, scheduler, tools::ToolSpec, wait_for, wait_for_turns,
};

fn codex(brief: &'static str) -> TestProject {
  project(&Setup::new(brief))
}

fn with(provider: AgentProvider, brief: &'static str) -> TestProject {
  project(&Setup {
    provider,
    ..Setup::new(brief)
  })
}

/// Stores a person's message to the task, as the desktop does.
fn send(root: &Path, task: TaskId, run: Option<RunId>, text: &str) -> Message {
  let message = Message::from_person(
    task,
    run,
    Recipient::Orchestrator,
    MessageBody::new(text).expect("a message"),
    Timestamp::now(),
  );
  ProjectDb::open(root)
    .expect("open")
    .send_message(IdempotencyKey::generate(), &message)
    .expect("stored")
}

fn conversation(root: &Path, task: TaskId) -> Vec<Message> {
  ProjectDb::open(root).expect("open").messages(task).expect("messages")
}

fn release(root: &Path) {
  std::fs::write(root.join(".orchester").join("fake-release"), "go").expect("release");
}

/// The agent's messages of the conversation, with their kind.
fn agent_messages(messages: &[Message]) -> Vec<(AgentMessage, String)> {
  messages
    .iter()
    .filter_map(|message| match message.sender() {
      Sender::Agent { kind, .. } => Some((*kind, message.body().as_str().to_owned())),
      Sender::Person { .. } => None,
    })
    .collect()
}

/// Waits until no task executes a run of `scheduler`.
async fn idle(scheduler: &Scheduler) {
  let settled = async {
    while !scheduler.active_runs().is_empty() {
      tokio::time::sleep(Duration::from_millis(20)).await;
    }
  };
  tokio::time::timeout(PATIENCE, settled)
    .await
    .expect("the run's task ends while it waits");
}

fn ended(run: &Run) -> bool {
  run.status().is_terminal()
}

/// The questions the scheduler announced, by task key and agent.
fn questions(events: &mut mpsc::UnboundedReceiver<Event>) -> Vec<(String, String)> {
  let mut found = Vec::new();
  while let Ok(event) = events.try_recv() {
    if let Event::QuestionAsked { task_key, agent, .. } = event {
      found.push((task_key, agent));
    }
  }
  found
}

#[tokio::test(flavor = "multi_thread")]
async fn conversation_a_message_reaches_the_working_agent_after_its_turn() {
  let project = codex("Wait for release.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  wait_for_turns(root, &mut events, id, 1).await;
  let message = send(root, project.task, Some(id), "Also cover the cancelled dialog.");
  scheduler
    .deliver_messages(id)
    .await
    .expect("taken at the next boundary");
  release(root);
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 2, "the step went on to read the message");
  assert_eq!(invocations[1].session, invocations[0].session);
  let prompt = reported(&db, id, "prompt| ");
  assert!(
    prompt
      .iter()
      .any(|line| line == "While you worked on this step, the person wrote to you:"),
    "{prompt:?}"
  );
  assert!(prompt.iter().any(|line| line == "Also cover the cancelled dialog."));
  let stored = conversation(root, project.task);
  assert!(matches!(
    stored[0].delivery(),
    Some(Delivery::Acknowledged { invocation, .. }) if invocation == invocations[1].id
  ));
  assert_eq!(stored[0].id(), message.id());
  // The resumed session's final message is its reply.
  assert_eq!(agent_messages(&stored), [(AgentMessage::Reply, "Done.".to_owned())]);
  // Its work before reading the message is a checkpoint of its own.
  let log = git(root, &["log", "--format=%s", run.branch().as_str()]);
  assert!(log.contains("saved to read messages"), "{log}");
  let readme = git(root, &["show", &format!("{}:README.md", run.branch())]);
  assert_eq!(readme, "hi\nhello\ncontinued");
}

async fn a_question_waits_and_the_answer_resumes_the_session(provider: AgentProvider) {
  let project = with(provider, "Ask first.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  let waiting = wait_for(root, &mut events, id, Run::waits_for_a_person).await;
  let implement = StepKey::new("implement").expect("key");
  let status = waiting.latest_execution(&implement).map(|execution| execution.status());
  assert_eq!(status, Some(StepStatus::Waiting(WaitReason::Message)));
  let asked = conversation(root, project.task);
  assert_eq!(
    agent_messages(&asked),
    [(AgentMessage::Question, "Keep email sign-in?".to_owned())]
  );
  assert!(asked[0].awaits_answer(&waiting, &asked));
  assert_eq!(questions(&mut events), [("ORC-1".to_owned(), "Builder".to_owned())]);
  // The waiting run holds no agent slot: its task has ended.
  idle(&scheduler).await;

  let answer = send(root, project.task, Some(id), "Yes, keep it.");
  scheduler.deliver_messages(id).await.expect("the step resumes");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 2);
  assert_eq!(invocations[1].session, invocations[0].session);
  let sources = db
    .invocation_prompt(invocations[1].id)
    .expect("read")
    .expect("a prompt");
  assert_eq!(sources.event, InjectionEvent::Resume);
  let prompt = reported(&db, id, "prompt| ");
  let asked_at = prompt
    .iter()
    .position(|line| line == "You asked the person:")
    .expect("the answer's lead");
  assert_eq!(prompt[asked_at + 2], "Keep email sign-in?", "{prompt:?}");
  assert!(prompt.iter().any(|line| line == "Yes, keep it."), "{prompt:?}");
  let stored = conversation(root, project.task);
  let answered = stored
    .iter()
    .find(|message| message.id() == answer.id())
    .expect("the answer");
  assert!(matches!(answered.delivery(), Some(Delivery::Acknowledged { .. })));
  assert!(!stored.iter().any(|message| message.awaits_answer(&run, &stored)));
  assert_eq!(agent_messages(&stored).len(), 2, "the question, then the reply");
}

#[tokio::test(flavor = "multi_thread")]
async fn conversation_a_codex_question_waits_and_the_answer_resumes_the_session() {
  a_question_waits_and_the_answer_resumes_the_session(AgentProvider::CodexCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn conversation_a_claude_question_waits_and_the_answer_resumes_the_session() {
  a_question_waits_and_the_answer_resumes_the_session(AgentProvider::ClaudeCodeCli).await;
}

async fn ask_agent_follows_up_a_run_ready_for_review(provider: AgentProvider) {
  let project = with(provider, "Append one line.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  let ready = wait_for(root, &mut events, id, ended).await;
  assert_eq!(ready.status(), &RunStatus::Succeeded);
  let reviewed = ready.last_checkpoint().expect("a checkpoint").revision().clone();
  idle(&scheduler).await;

  send(root, project.task, Some(id), "Add a second line.");
  scheduler.deliver_messages(id).await.expect("followed up");
  let working = ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("run");
  assert_eq!(working.status(), &RunStatus::Running);
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded, "ready for review again");
  assert_eq!(
    run.current_attempt().map(|attempt| attempt.reason().clone()),
    Some(AttemptReason::FollowUp {
      step: StepKey::new("implement").expect("key")
    })
  );
  let checkpoint = run.last_checkpoint().expect("a checkpoint");
  assert_ne!(checkpoint.revision(), &reviewed, "the follow-up made a new checkpoint");
  let readme = git(
    root,
    &["show", &format!("{}:README.md", checkpoint.revision().as_str())],
  );
  assert_eq!(readme, "hi\nhello\ncontinued");
  let db = ProjectDb::open(root).expect("open");
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations.len(), 2);
  assert_eq!(
    invocations[1].session, invocations[0].session,
    "the step's session continued"
  );
  let prompt = reported(&db, id, "prompt| ");
  assert!(
    prompt
      .iter()
      .any(|line| line.starts_with("You completed this step and the run is ready for review.")),
    "{prompt:?}"
  );
  let stored = conversation(root, project.task);
  assert!(matches!(stored[0].delivery(), Some(Delivery::Acknowledged { .. })));
  assert_eq!(agent_messages(&stored).len(), 1, "the follow-up's reply");
}

#[tokio::test(flavor = "multi_thread")]
async fn conversation_ask_agent_follows_up_a_codex_run_ready_for_review() {
  ask_agent_follows_up_a_run_ready_for_review(AgentProvider::CodexCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn conversation_ask_agent_follows_up_a_claude_run_ready_for_review() {
  ask_agent_follows_up_a_run_ready_for_review(AgentProvider::ClaudeCodeCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn conversation_a_run_waiting_for_an_answer_cancels_and_takes_no_message_after() {
  let project = codex("Ask first.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  wait_for(root, &mut events, id, Run::waits_for_a_person).await;
  idle(&scheduler).await;
  scheduler.cancel_run(id).await.expect("cancelled");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Cancelled);
  let status = run
    .latest_execution(&StepKey::new("implement").expect("key"))
    .map(|execution| execution.status());
  assert_eq!(status, Some(StepStatus::Skipped));
  send(root, project.task, Some(id), "Yes, keep it.");
  assert_eq!(scheduler.deliver_messages(id).await, Err(Rejection::RunEnded));
  assert_eq!(
    ProjectDb::open(root)
      .expect("open")
      .invocations(id)
      .expect("invocations")
      .len(),
    1
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn conversation_an_answer_written_without_a_runner_resumes_the_step_when_one_attaches() {
  let project = codex("Ask first.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  wait_for(root, &mut events, id, Run::waits_for_a_person).await;
  idle(&scheduler).await;
  drop(scheduler);
  // No runner is attached: the desktop stores the answer, and nobody is told.
  send(root, project.task, Some(id), "Yes, keep it.");
  let (next, mut next_events) = support::scheduler(root);
  next
    .attach(AttachSession::Reconnect, std::num::NonZeroU32::MIN)
    .expect("attached");
  let run = wait_for(root, &mut next_events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.invocations(id).expect("invocations").len(), 2);
  let prompt = reported(&db, id, "prompt| ");
  assert!(prompt.iter().any(|line| line == "Yes, keep it."), "{prompt:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn conversation_a_message_no_agent_read_before_the_run_succeeded_follows_it_up() {
  let project = project(&Setup {
    steps: vec![
      StepSpec::new("implement", "Implement", "Append one line.", &[]),
      StepSpec::new("checks", "Checks", "", &["implement"]).command(ToolSpec::test_command(&["sleep", "2"])),
    ],
    ..Setup::new("Say hello.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  let checks = StepKey::new("checks").expect("key");
  wait_for(root, &mut events, id, |run| {
    run.latest_execution(&checks).map(|execution| execution.status()) == Some(StepStatus::Running)
  })
  .await;
  // Only a command works: no agent reads the message before the run succeeds.
  send(root, project.task, Some(id), "Add a second line.");
  scheduler.deliver_messages(id).await.expect("taken");
  let run = wait_for(root, &mut events, id, |run| {
    ended(run)
      && matches!(
        run.current_attempt().map(|attempt| attempt.reason()),
        Some(AttemptReason::FollowUp { .. })
      )
  })
  .await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let stored = conversation(root, project.task);
  assert!(matches!(stored[0].delivery(), Some(Delivery::Acknowledged { .. })));
  // The follow-up's task counted the run as executed until it succeeded again.
  idle(&scheduler).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn conversation_a_message_written_before_the_run_reaches_its_first_step() {
  let project = codex("Append one line.");
  let root = project.repo.path();
  let early = send(root, project.task, None, "Keep the email sign-in.");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let prompt = reported(&db, id, "prompt| ");
  let heading = prompt
    .iter()
    .position(|line| line == "# Messages from the person")
    .expect("the messages in the task part");
  assert!(prompt[heading..].iter().any(|line| line == "Keep the email sign-in."));
  assert_eq!(db.invocations(id).expect("invocations").len(), 1);
  let stored = conversation(root, project.task);
  assert_eq!(stored[0].id(), early.id());
  assert_eq!(stored[0].run(), Some(id));
  assert!(matches!(stored[0].delivery(), Some(Delivery::Acknowledged { .. })));
}
