//! *Compact* of an Orchestrator conversation's session (ORC-309), end to end against the fake CLIs at their boundary:
//! Claude Code's `/compact` (`tests/support/fake_claude_chat.rs`) and Codex App Server's `thread/compact/start`
//! (`tests/support/fake_codex_chat.rs`). A compaction is kept running, then done with the size before and after, or
//! failed with why; a conversation without a session or with a turn under way is refused. A turn Orchester started
//! by itself compacts a session that is nearly full before it runs; a person's own message never does (OA-05).
//! Run with `cargo test -p orchester-runner --features test-agent --test chat_compact compact`.

mod support;

use std::{fs, path::Path};

use orchester_core::{
  connection::{AgentProvider, ConnectionAccount, context_use::ContextUse},
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, RunId},
  orchestrator::{
    AgentWait, Chat, ChatEvent, CompactionState, EntryKind, OrchestratorSettings, TurnFailure, TurnState, WaitKind,
  },
  resource::ModelId,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_ipc::{ChatToken, ChatTools, Event, Rejection};
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, scheduler};

/// A project whose Orchestrator runs on `provider` with `model`.
fn on(provider: AgentProvider, model: &str) -> TestProject {
  let project = project(&Setup {
    provider,
    ..Setup::new("Not used by the chat.")
  });
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  let connection = db.connections().expect("connections")[0].id();
  let model = ModelId::new(model).expect("model");
  db.publish_orchestrator(&OrchestratorSettings::new(connection, model, None), Timestamp::now())
    .expect("settings");
  project
}

fn tools() -> ChatTools {
  ChatTools {
    url: "http://127.0.0.1:9/mcp".to_owned(),
    token: ChatToken::new("chat-token-7f3a9c".to_owned()),
  }
}

/// A conversation with one answered message whose session is `session` on the project's connection, its context at
/// 172k of 200k.
fn conversation(root: &Path, provider: AgentProvider, session: Option<&str>) -> ChatId {
  let mut db = ProjectDb::open(root).expect("open");
  let chat = ChatId::generate();
  let body = MessageBody::new("What is running?").expect("text");
  let message = ChatMessageId::generate();
  db.keep_chat_message(
    &Chat::start(chat, &body, Timestamp::now()),
    (message, &body),
    Timestamp::now(),
  )
  .expect("kept");
  db.end_chat_turn(message, &TurnState::Answered, Timestamp::now())
    .expect("answered");
  if let Some(session) = session {
    let connection = db.connections().expect("connections")[0].id();
    let account = ConnectionAccount::Default.id(provider).expect("an account");
    db.set_chat_session(chat, session, (connection, account))
      .expect("session");
    db.set_chat_context(chat, &ContextUse::reported(172_000, 200_000))
      .expect("context");
  }
  chat
}

/// The compactions `chat` keeps, once none of them runs.
async fn compactions(
  root: &Path,
  events: &mut mpsc::UnboundedReceiver<Event>,
  chat: ChatId,
) -> Vec<(Option<u64>, CompactionState)> {
  let wait = async {
    loop {
      let kept: Vec<(Option<u64>, CompactionState)> = ProjectDb::open(root)
        .expect("open")
        .chat_entries(chat)
        .expect("entries")
        .into_iter()
        .filter_map(|entry| match entry.kind {
          EntryKind::Event(ChatEvent::Compaction { before, state }) => Some((before, state)),
          _ => None,
        })
        .collect();
      if kept.iter().all(|(_, state)| *state != CompactionState::Running) {
        return kept;
      }
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, wait)
    .await
    .expect("the compaction ends in time")
}

fn context(root: &Path, chat: ChatId) -> Option<ContextUse> {
  ProjectDb::open(root).expect("open").chat_context(chat).expect("read")
}

fn log(root: &Path) -> String {
  fs::read_to_string(root.join(".orchester").join("fake-chat.log")).expect("the fake logged its start")
}

#[tokio::test(flavor = "multi_thread")]
async fn compact_on_claude_code_runs_its_own_compact_and_keeps_the_event() {
  let project = on(AgentProvider::ClaudeCodeCli, "haiku");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = conversation(root, AgentProvider::ClaudeCodeCli, Some("session-long"));
  scheduler.compact_chat(chat, tools()).await.expect("it compacts");
  assert_eq!(
    compactions(root, &mut events, chat).await,
    [(Some(172_000), CompactionState::Done { after: None })],
    "Claude Code reports no size after its compaction"
  );
  assert_eq!(
    context(root, chat),
    None,
    "the reading before the compaction is forgotten until the next turn reports one"
  );
  let started = log(root);
  assert!(
    started.contains("--resume session-long") && !started.contains("--disable-slash-commands"),
    "/compact resumes the session with slash commands on: {started}"
  );
  let replies = ProjectDb::open(root)
    .expect("open")
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter(|entry| matches!(entry.kind, EntryKind::Reply { .. }))
    .count();
  assert_eq!(replies, 0, "a compaction says nothing in the conversation");
}

#[tokio::test(flavor = "multi_thread")]
async fn compact_on_claude_code_that_cannot_says_why() {
  let project = on(AgentProvider::ClaudeCodeCli, "haiku");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = conversation(root, AgentProvider::ClaudeCodeCli, Some("cannot-compact"));
  scheduler.compact_chat(chat, tools()).await.expect("it starts");
  assert_eq!(
    compactions(root, &mut events, chat).await,
    [(
      Some(172_000),
      CompactionState::Failed {
        reason: TurnFailure::Failed {
          reason: "Error: No messages to compact".to_owned()
        }
      }
    )]
  );
  assert_eq!(
    context(root, chat),
    Some(ContextUse::reported(172_000, 200_000)),
    "the session is as it was"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn compact_needs_a_session_and_no_turn_under_way() {
  let project = on(AgentProvider::ClaudeCodeCli, "haiku");
  let root = project.repo.path();
  let (scheduler, _events) = scheduler(root);
  let fresh = conversation(root, AgentProvider::ClaudeCodeCli, None);
  assert_eq!(
    scheduler.compact_chat(fresh, tools()).await,
    Err(Rejection::NothingToCompact)
  );
  assert_eq!(
    scheduler.compact_chat(ChatId::generate(), tools()).await,
    Err(Rejection::NothingToCompact),
    "no such conversation"
  );

  let chat = conversation(root, AgentProvider::ClaudeCodeCli, Some("session-busy"));
  let mut db = ProjectDb::open(root).expect("open");
  let kept = db.chat(chat).expect("read").expect("kept");
  let thinking = ChatMessageId::generate();
  let body = MessageBody::new("Keep thinking about the queue.").expect("text");
  db.keep_chat_message(&kept, (thinking, &body), Timestamp::now())
    .expect("kept");
  scheduler
    .answer_in_chat(thinking, tools())
    .await
    .expect("the turn starts");
  assert_eq!(
    scheduler.compact_chat(chat, tools()).await,
    Err(Rejection::ChatBusy),
    "not while a turn is under way"
  );
  scheduler.stop_chat(chat).expect("stopped");
}

#[tokio::test(flavor = "multi_thread")]
async fn compact_on_codex_compacts_the_thread_and_lowers_the_reading() {
  let project = on(AgentProvider::CodexCli, "gpt-5.5");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = conversation(root, AgentProvider::CodexCli, Some("thread-long"));
  scheduler.compact_chat(chat, tools()).await.expect("it compacts");
  assert_eq!(
    compactions(root, &mut events, chat).await,
    [(Some(172_000), CompactionState::Done { after: Some(41_000) })]
  );
  assert_eq!(
    context(root, chat),
    Some(ContextUse::reported(41_000, 200_000)),
    "the reading Codex reported after it"
  );
  assert!(log(root).contains("compact: thread-long"), "{}", log(root));

  let empty = conversation(root, AgentProvider::CodexCli, Some("thread-empty-1"));
  scheduler.compact_chat(empty, tools()).await.expect("it starts");
  let ended = compactions(root, &mut events, empty).await;
  assert!(
    matches!(&ended[..], [(Some(172_000), CompactionState::Failed { reason: TurnFailure::Failed { reason } })]
      if reason.contains("no history to compact")),
    "{ended:?}"
  );
}

/// A wait of an agent kept in `chat`, and the turn it started by itself.
fn wait_turn(root: &Path, chat: ChatId) -> ChatMessageId {
  let mut db = ProjectDb::open(root).expect("open");
  let kept = db.chat(chat).expect("read").expect("the conversation");
  let wait = AgentWait {
    kind: WaitKind::Message,
    task: "ORC-1".to_owned(),
    title: "Say hello".to_owned(),
    run: RunId::generate(),
    number: 1,
    step: Some("Implement".to_owned()),
    agent: Some("Builder".to_owned()),
    place: None,
    others: 0,
    link: "/project/p/tasks/ORC-1/conversation".to_owned(),
    automatic: true,
    continuing: None,
    merging: None,
  };
  db.keep_agent_wait(&kept, ("message:x:1", &wait), Timestamp::now())
    .expect("kept");
  let seq = db.automatic_waits_without_turn().expect("waits")[0].seq;
  let message = ChatMessageId::generate();
  assert!(db.start_wait_turn((chat, seq), message).expect("started"));
  message
}

/// The turn of `message` once it is no longer thinking.
async fn ended(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, message: ChatMessageId) -> TurnState {
  let wait = async {
    loop {
      let turn = ProjectDb::open(root)
        .expect("open")
        .chat_turn(message)
        .expect("read")
        .expect("kept")
        .turn;
      if !turn.is_thinking() {
        return turn;
      }
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, wait)
    .await
    .expect("the turn ends in time")
}

#[tokio::test(flavor = "multi_thread")]
async fn compact_runs_first_in_a_turn_orchester_started_once_the_session_is_nearly_full() {
  let project = on(AgentProvider::ClaudeCodeCli, "haiku");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = conversation(root, AgentProvider::ClaudeCodeCli, Some("session-long"));
  let started = wait_turn(root, chat);
  scheduler
    .answer_in_chat(started, tools())
    .await
    .expect("the turn starts");
  assert_eq!(ended(root, &mut events, started).await, TurnState::Answered);
  assert_eq!(
    compactions(root, &mut events, chat).await,
    [(Some(172_000), CompactionState::Done { after: None })],
    "the session held 172k of 200k: compacted before the turn ran"
  );
  let entries = ProjectDb::open(root)
    .expect("open")
    .chat_entries(chat)
    .expect("entries");
  let compacted = entries
    .iter()
    .position(|entry| matches!(entry.kind, EntryKind::Event(ChatEvent::Compaction { .. })))
    .expect("the compaction");
  let replied = entries
    .iter()
    .position(|entry| matches!(entry.kind, EntryKind::Reply { .. }))
    .expect("the turn's reply");
  assert!(compacted < replied, "the compaction, then the answer");
  let log = log(root);
  let started: Vec<&str> = log.lines().filter(|line| line.starts_with("argv: ")).collect();
  assert_eq!(started.len(), 2, "one CLI to compact, one to answer: {log}");
  assert!(
    !started[0].contains("--disable-slash-commands") && started[1].contains("--disable-slash-commands"),
    "/compact ran first, with slash commands on, then the turn: {started:?}"
  );
  assert!(
    started.iter().all(|line| line.contains("--resume session-long")),
    "both resume the conversation's session: {started:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn compact_that_fails_first_does_not_keep_the_turn_from_running() {
  let project = on(AgentProvider::ClaudeCodeCli, "haiku");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = conversation(root, AgentProvider::ClaudeCodeCli, Some("cannot-compact"));
  // Exactly the share a session is compacted from.
  ProjectDb::open(root)
    .expect("open")
    .set_chat_context(chat, &ContextUse::reported(140_000, 200_000))
    .expect("context");
  let started = wait_turn(root, chat);
  scheduler
    .answer_in_chat(started, tools())
    .await
    .expect("the turn starts");
  assert_eq!(
    ended(root, &mut events, started).await,
    TurnState::Answered,
    "the turn ran"
  );
  let kept = compactions(root, &mut events, chat).await;
  assert!(
    matches!(kept.as_slice(), [(Some(140_000), CompactionState::Failed { .. })]),
    "after the compaction that failed: {kept:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn compact_runs_first_for_no_message_of_the_persons_and_no_session_with_room() {
  let project = on(AgentProvider::ClaudeCodeCli, "haiku");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  // A person's message in a session that is nearly full: they see the meter, and decide.
  let full = conversation(root, AgentProvider::ClaudeCodeCli, Some("session-long"));
  let mut db = ProjectDb::open(root).expect("open");
  let kept = db.chat(full).expect("read").expect("the conversation");
  let (asked, body) = (ChatMessageId::generate(), MessageBody::new("And now?").expect("text"));
  db.keep_chat_message(&kept, (asked, &body), Timestamp::now())
    .expect("kept");
  scheduler.answer_in_chat(asked, tools()).await.expect("the turn starts");
  assert_eq!(ended(root, &mut events, asked).await, TurnState::Answered);
  assert!(compactions(root, &mut events, full).await.is_empty());

  // A turn Orchester started in a session with room left.
  let roomy = conversation(root, AgentProvider::ClaudeCodeCli, Some("session-short"));
  db.set_chat_context(roomy, &ContextUse::reported(139_000, 200_000))
    .expect("context");
  let started = wait_turn(root, roomy);
  scheduler
    .answer_in_chat(started, tools())
    .await
    .expect("the turn starts");
  assert_eq!(ended(root, &mut events, started).await, TurnState::Answered);
  assert!(
    compactions(root, &mut events, roomy).await.is_empty(),
    "69 % of the window is not nearly full"
  );
}
