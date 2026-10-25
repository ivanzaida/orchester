//! Send now and Send after this turn (ORC-61) end to end against `orchester-fake-codex` and
//! `orchester-fake-claude`, which take a message the way Codex 0.159.2 and Claude Code 2.1.286 did in
//! the probe of 2026-10-02: a message sent now reaches the agent at work after the tool call it is
//! running (delivered mid-turn, in the same invocation); one whose turn ended first is delivered at
//! the end of the turn (Codex: the session continues to read it, or, when Codex had accepted it, it
//! kept it as the turn ended and it is delivered once; Claude Code: its next turn); one
//! Codex refuses, or that Claude Code does not read, is not delivered and goes no further, unless automation
//! sent it: then it is sent after the turn; a message sent after the turn stays queued while the agent works
//! and is delivered when its turn ends.
//! Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test send_now send_`. The live
//! checks on the real CLIs are ignored: they run with `-- --ignored` and the CLI's program in
//! `ORCHESTER_LIVE_CODEX` or `ORCHESTER_LIVE_CLAUDE` (`docs/evidence/orc-61-send-now-2026-10-02.md`).

mod support;

use std::path::Path;

use orchester_core::{
  connection::{AgentProvider, stream::AgentEvent},
  conversation::{Arrival, Delivery, Message, MessageBody, Recipient, Timing, Undelivered},
  id::{IdempotencyKey, RunId, TaskId},
  run::{Run, RunStatus},
  store::{ActivityStore, ConversationStore, RunStore},
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, reported, scheduler, wait_for, wait_for_turns};

fn with(provider: AgentProvider, brief: &'static str) -> TestProject {
  project(&Setup {
    provider,
    ..Setup::new(brief)
  })
}

/// Stores a person's message to the run, as the desktop does, sent as `timing` says.
fn send(root: &Path, task: TaskId, run: RunId, text: &str, timing: Timing) -> Message {
  let message = Message::from_person(
    task,
    Some(run),
    Recipient::Orchestrator,
    MessageBody::new(text).expect("a message"),
    Timestamp::now(),
  )
  .sent(timing)
  .expect("a person's message");
  ProjectDb::open(root)
    .expect("open")
    .send_message(IdempotencyKey::generate(), &message)
    .expect("stored")
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

/// Waits until the person's message is no longer queued.
async fn left_the_queue(root: &Path, task: TaskId, events: &mut mpsc::UnboundedReceiver<Event>) -> Delivery {
  let moved = async {
    loop {
      match delivery(root, task) {
        Some(Delivery::Queued) | None => {
          events.recv().await.expect("the scheduler notifies");
        }
        Some(delivery) => return delivery,
      }
    }
  };
  tokio::time::timeout(PATIENCE, moved)
    .await
    .expect("the message moves on in time")
}

/// The agent's messages in the run's transcript.
fn agent_said(root: &Path, run: RunId) -> Vec<String> {
  let db = ProjectDb::open(root).expect("open");
  db.activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::Message { text } => Some(text),
      _ => None,
    })
    .collect()
}

/// The run's invocations.
fn invocations(root: &Path, run: RunId) -> usize {
  ProjectDb::open(root)
    .expect("open")
    .invocations(run)
    .expect("invocations")
    .len()
}

/// Sends a message now while the agent's command runs: it is delivered mid-turn, after the command,
/// in the same invocation, and the agent's turn read it.
async fn takes_a_message_mid_turn(provider: AgentProvider) {
  let project = with(provider, "Wait for a message.");
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  send(root, project.task, id, "Also cover the cancelled dialog.", Timing::Now);
  scheduler.deliver_messages(id).await.expect("taken");
  let delivered = left_the_queue(root, project.task, &mut events).await;
  assert!(
    matches!(
      delivered,
      Delivery::Delivered {
        arrival: Arrival::MidTurn,
        ..
      } | Delivery::Acknowledged {
        arrival: Arrival::MidTurn,
        ..
      }
    ),
    "{delivered:?}"
  );
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(invocations(root, id), 1, "read in the turn it was sent in");
  assert!(
    agent_said(root, id).contains(&"Noted: Also cover the cancelled dialog.".to_owned()),
    "{:?}",
    agent_said(root, id)
  );
  assert!(matches!(
    delivery(root, project.task),
    Some(Delivery::Acknowledged {
      arrival: Arrival::MidTurn,
      ..
    })
  ));
}

#[tokio::test(flavor = "multi_thread")]
async fn send_now_codex_takes_a_message_mid_turn_after_the_running_command() {
  takes_a_message_mid_turn(AgentProvider::CodexCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn send_now_claude_takes_a_message_mid_turn_after_the_running_tool_call() {
  takes_a_message_mid_turn(AgentProvider::ClaudeCodeCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn send_now_codex_delivers_at_the_end_of_a_turn_that_ended_first() {
  // The fake runs its command without reading its input: the turn ends before Codex sees the
  // steer, and the session continues to read the message.
  let project = with(AgentProvider::CodexCli, "Wait for release.");
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  send(root, project.task, id, "Rename the test.", Timing::Now);
  scheduler.deliver_messages(id).await.expect("taken");
  release(root);
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(invocations(root, id), 2, "the session continued to read it");
  let stored = ProjectDb::open(root)
    .expect("open")
    .messages(project.task)
    .expect("messages");
  assert_eq!(stored[0].timing(), Timing::Now);
  assert!(
    matches!(
      stored[0].delivery(),
      Some(Delivery::Acknowledged {
        arrival: Arrival::TurnEnd,
        ..
      })
    ),
    "{:?}",
    stored[0].delivery()
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn send_now_codex_accepted_but_not_taken_is_delivered_once_at_the_turn_end() {
  // Codex accepts the steer, then the turn finishes before the model reads it: Codex records it into
  // the thread as the turn ends, so the runner must not give it to a next turn as well.
  let project = with(AgentProvider::CodexCli, "Keep a message for the turn's end.");
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  send(root, project.task, id, "Rename the test.", Timing::Now);
  scheduler.deliver_messages(id).await.expect("taken");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(invocations(root, id), 1, "not sent again in a turn of its own");
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

#[tokio::test(flavor = "multi_thread")]
async fn send_now_claude_takes_a_message_as_its_next_turn_when_the_turn_ended_first() {
  let project = with(AgentProvider::ClaudeCodeCli, "Finish after release.");
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  send(root, project.task, id, "Rename the test.", Timing::Now);
  scheduler.deliver_messages(id).await.expect("taken");
  // Written to the CLI before its turn ends.
  let written = async {
    while !reported(&ProjectDb::open(root).expect("open"), id, "input: ").contains(&"Rename the test.".to_owned()) {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, written).await.expect("written in time");
  release(root);
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(invocations(root, id), 1, "Claude Code took it as its next turn");
  assert!(
    matches!(
      delivery(root, project.task),
      Some(Delivery::Acknowledged {
        arrival: Arrival::TurnEnd,
        ..
      })
    ),
    "{:?}",
    delivery(root, project.task)
  );
  assert!(
    agent_said(root, id)
      .iter()
      .any(|said| said.contains("Read after the turn: Rename the test.")),
    "{:?}",
    agent_said(root, id)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn send_now_codex_refused_is_not_delivered_and_goes_no_further() {
  let project = with(AgentProvider::CodexCli, "Refuse a message.");
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  send(root, project.task, id, "Use the new API.", Timing::Now);
  scheduler.deliver_messages(id).await.expect("taken");
  let refused = left_the_queue(root, project.task, &mut events).await;
  assert!(
    matches!(
      refused,
      Delivery::NotDelivered {
        reason: Undelivered::Refused,
        ..
      }
    ),
    "{refused:?}"
  );
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(invocations(root, id), 1, "the session did not continue for it");
  assert!(matches!(
    delivery(root, project.task),
    Some(Delivery::NotDelivered { .. })
  ));
}

/// Issue #56: automation's message sent now that Codex refuses to add to the running turn is not left for a
/// person to send again: it is queued for the turn's end (the conversation and `get_conversation` show it
/// queued, never not delivered), and the session continues to read it as the turn ends.
#[tokio::test(flavor = "multi_thread")]
async fn send_now_codex_refused_from_automation_is_sent_after_the_turn() {
  let project = with(AgentProvider::CodexCli, "Refuse a message.");
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  let message = Message::from_person(
    project.task,
    Some(id),
    Recipient::Orchestrator,
    MessageBody::new("Continue with the research.").expect("a message"),
    Timestamp::now(),
  )
  .sent(Timing::Now)
  .and_then(Message::sent_by_automation)
  .expect("a message on the person's side");
  ProjectDb::open(root)
    .expect("open")
    .send_message(IdempotencyKey::generate(), &message)
    .expect("stored");
  scheduler.deliver_messages(id).await.expect("taken");
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(
    invocations(root, id),
    2,
    "the session continued to read it after the turn"
  );
  let stored = ProjectDb::open(root)
    .expect("open")
    .messages(project.task)
    .expect("messages");
  let sent = stored
    .iter()
    .find(|stored| stored.id() == message.id())
    .expect("the message");
  assert_eq!(
    sent.timing(),
    Timing::AfterTurn,
    "queued for the turn's end, as Send after this turn"
  );
  assert!(
    matches!(
      sent.delivery(),
      Some(Delivery::Acknowledged {
        arrival: Arrival::TurnEnd,
        ..
      })
    ),
    "{:?}",
    sent.delivery()
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn send_now_claude_not_read_is_not_delivered_and_goes_no_further() {
  let project = with(AgentProvider::ClaudeCodeCli, "Finish after release. Ignore messages.");
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  // More than a pipe holds: the CLI, which reads nothing, cannot take it.
  let long = "Use the new API. ".repeat(80_000);
  send(root, project.task, id, &long, Timing::Now);
  scheduler.deliver_messages(id).await.expect("taken");
  let unread = left_the_queue(root, project.task, &mut events).await;
  assert!(
    matches!(
      unread,
      Delivery::NotDelivered {
        reason: Undelivered::Unwritable,
        ..
      }
    ),
    "{unread:?}"
  );
  release(root);
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(invocations(root, id), 1);
  assert!(matches!(
    delivery(root, project.task),
    Some(Delivery::NotDelivered { .. })
  ));
}

/// Sends a message after the turn while the agent's command runs: it stays queued until the turn
/// ends, then the session continues to read it.
async fn queues_a_message_after_the_turn(provider: AgentProvider) {
  let project = with(provider, "Wait for release.");
  let root = project.repo.path();
  let (scheduler, mut events, id) = started(&project).await;
  send(
    root,
    project.task,
    id,
    "Also cover the cancelled dialog.",
    Timing::AfterTurn,
  );
  scheduler.deliver_messages(id).await.expect("taken");
  assert_eq!(
    delivery(root, project.task),
    Some(Delivery::Queued),
    "queued while the agent works"
  );
  release(root);
  let run = wait_for(root, &mut events, id, ended).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(invocations(root, id), 2, "the session continued to read it");
  assert!(
    matches!(
      delivery(root, project.task),
      Some(Delivery::Acknowledged {
        arrival: Arrival::TurnEnd,
        ..
      })
    ),
    "{:?}",
    delivery(root, project.task)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn send_after_turn_codex_is_queued_until_the_turn_ends() {
  queues_a_message_after_the_turn(AgentProvider::CodexCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn send_after_turn_claude_is_queued_until_the_turn_ends() {
  queues_a_message_after_the_turn(AgentProvider::ClaudeCodeCli).await;
}

/// What a live check asks the real agent: a command that runs long enough to send it a message.
const LIVE_BRIEF: &str = "Run this exact command once and wait for it to finish: python -c \"import time; \
                          time.sleep(20); print('slept')\". Change no file. Then finish the step.";
/// How long a live check waits for the real agent.
const LIVE_PATIENCE: std::time::Duration = std::time::Duration::from_secs(300);

/// A project on the real agent CLI whose program the environment variable `program` names, on
/// `model`: a live check, run by hand with `--ignored` on the CLI's own sign-in.
fn live(provider: AgentProvider, program: &str, model: &'static str) -> TestProject {
  use orchester_core::{
    connection::{AgentConnection, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
    permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess},
    resource::RepositoryAccess,
    store::ConnectionStore,
  };
  let executable = std::env::var_os(program).unwrap_or_else(|| panic!("set {program} to the agent CLI's program"));
  // Codex confines any command to the worktree; Claude Code runs the listed ones.
  let commands = match provider {
    AgentProvider::ClaudeCodeCli => CommandPermission::Allowed(
      AllowedCommands::new(vec![CommandPrefix::new("python").expect("valid prefix")]).expect("valid list"),
    ),
    _ => CommandPermission::AnyInWorktree,
  };
  let project = project(&Setup {
    provider,
    model: Some(model),
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      commands,
      NetworkAccess::Off,
    )),
    ..Setup::new(LIVE_BRIEF)
  });
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  for connection in db.connections().expect("connections") {
    if let Ok(mut agent) = AgentConnection::try_from(connection) {
      agent
        .set_executable(executable.clone().into())
        .expect("an absolute program");
      // The CLI's own sign-in, as the connection check would find it.
      agent
        .record_check(ConnectionCheck {
          at: Timestamp::now(),
          result: CheckResult::Available {
            version: "live".into(),
            sign_in: SignIn::SignedIn,
            catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
          },
        })
        .expect("an agent check fits a CLI connection");
      db.save_connection(&agent.into_connection()).expect("saved");
    }
  }
  project
}

/// The real agent runs its command; a message sent now reaches it mid-turn and its result says so.
async fn live_send_now(provider: AgentProvider, program: &str, model: &'static str) {
  let project = live(provider, program, model);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let commanded = async {
    loop {
      let db = ProjectDb::open(root).expect("open");
      let activity = db.activity(id, 0).expect("activity");
      if activity
        .iter()
        .any(|entry| matches!(entry.event, AgentEvent::CommandStarted { .. }))
      {
        return;
      }
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(LIVE_PATIENCE, commanded)
    .await
    .expect("the agent runs its command in time");
  send(
    root,
    project.task,
    id,
    "While you work: include the word BANANA in your step result's summary.",
    Timing::Now,
  );
  scheduler.deliver_messages(id).await.expect("taken");
  let finished = async {
    loop {
      let run = ProjectDb::open(root)
        .expect("open")
        .run(id)
        .expect("run")
        .expect("the run");
      if ended(&run) {
        return run;
      }
      // The run is read again on each notice, and every second in case one was missed.
      let _notice = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv()).await;
    }
  };
  let Ok(run) = tokio::time::timeout(LIVE_PATIENCE, finished).await else {
    let activity = ProjectDb::open(root).expect("open").activity(id, 0).expect("activity");
    let events: Vec<String> = activity.iter().map(|entry| format!("{:?}", entry.event)).collect();
    panic!(
      "the run did not end in time; {:?}; transcript:\n{}",
      delivery(root, project.task),
      events.join("\n")
    );
  };
  let delivered = delivery(root, project.task);
  assert!(
    matches!(
      delivered,
      Some(Delivery::Acknowledged {
        arrival: Arrival::MidTurn,
        ..
      })
    ),
    "{delivered:?}"
  );
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(invocations(root, id), 1, "read in the turn it was sent in");
  let said = agent_said(root, id).join("\n");
  assert!(said.contains("BANANA"), "{said}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live: runs the real Codex CLI named by ORCHESTER_LIVE_CODEX on its own sign-in"]
async fn send_now_live_codex_takes_a_message_mid_turn() {
  live_send_now(AgentProvider::CodexCli, "ORCHESTER_LIVE_CODEX", "gpt-6.1-sol").await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "live: runs the real Claude Code CLI named by ORCHESTER_LIVE_CLAUDE on its own sign-in"]
async fn send_now_live_claude_takes_a_message_mid_turn() {
  live_send_now(AgentProvider::ClaudeCodeCli, "ORCHESTER_LIVE_CLAUDE", "haiku").await;
}
