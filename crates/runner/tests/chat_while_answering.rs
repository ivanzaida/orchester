//! Writing to the Orchestrator while it answers (ORC-92), end to end against `orchester-fake-claude` and
//! `orchester-fake-codex` as the chat's CLI: a message sent now joins the answer under way (Claude Code takes it from
//! its stream-json input, Codex from `turn/steer`) and the answer says it read it; one the CLI refuses, or that no
//! answer under way could take, waits for a turn of its own; a message queued after the answer, or to go next once it
//! is stopped (*Stop and send*), starts its turn only once the answer under way ended, in order, and is answered then.
//! The app's follower starts a queued turn (`orchester_workspace::orchestrator::queue`); the tests do as it does.
//! Run with `cargo test -p orchester-runner --features test-agent --test chat_while_answering while_answering`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::AgentProvider,
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, EntryKind, OrchestratorSettings, Queued, TurnState},
  resource::ModelId,
  run::Answerer,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_ipc::{ChatToken, ChatTools, Event};
use orchester_store::{ProjectDb, StartedQueued};
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, scheduler};

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

/// Keeps `text` as a new message of `chat` (started with it when new), its turn as `state` says.
fn keep(root: &Path, chat: ChatId, text: &str, state: &TurnState) -> ChatMessageId {
  let mut db = ProjectDb::open(root).expect("open");
  let body = MessageBody::new(text).expect("text");
  let kept = db
    .chat(chat)
    .expect("chat")
    .unwrap_or_else(|| Chat::start(chat, &body, Timestamp::now()));
  let message = ChatMessageId::generate();
  assert!(
    db.keep_chat_message_as(
      &kept,
      (message, &body),
      (&[], Answerer::Person),
      (state, Timestamp::now())
    )
    .expect("kept")
  );
  message
}

fn turn(root: &Path, message: ChatMessageId) -> TurnState {
  ProjectDb::open(root)
    .expect("open")
    .chat_turn(message)
    .expect("read")
    .expect("kept")
    .turn
}

/// Waits until the turn of `message` holds as `done` says.
async fn until(
  root: &Path,
  events: &mut mpsc::UnboundedReceiver<Event>,
  message: ChatMessageId,
  done: impl Fn(&TurnState) -> bool,
) -> TurnState {
  let wait = async {
    loop {
      let now = turn(root, message);
      if done(&now) {
        return now;
      }
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, wait).await.expect("in time")
}

fn replies(root: &Path, chat: ChatId) -> Vec<String> {
  ProjectDb::open(root)
    .expect("open")
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .filter_map(|entry| match entry.kind {
      EntryKind::Reply { text, .. } => Some(text),
      _ => None,
    })
    .collect()
}

async fn replied(root: &Path, events: &mut mpsc::UnboundedReceiver<Event>, chat: ChatId, count: usize) {
  let wait = async {
    while replies(root, chat).len() < count {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, wait).await.expect("the reply comes");
}

/// The answer under way takes a message sent now: it joins that turn, and the answer says it read it.
async fn send_now_joins_the_answer(provider: AgentProvider, model: &str) {
  let project = on(provider, model);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let first = keep(root, chat, "Wait for a message.", &TurnState::Thinking);
  scheduler.answer_in_chat(first, tools()).await.expect("the turn starts");
  replied(root, &mut events, chat, 1).await;
  let now = keep(root, chat, "Also check the queue.", &TurnState::Steering);
  scheduler.steer_chat(now).expect("handed to the answer");
  assert_eq!(
    until(root, &mut events, first, |state| !state.is_thinking()).await,
    TurnState::Answered
  );
  assert_eq!(turn(root, now), TurnState::Joined, "the answer took it");
  assert!(
    replies(root, chat).contains(&"Noted: Also check the queue.".to_owned()),
    "{:?}",
    replies(root, chat)
  );
  assert!(scheduler.steer_chat(now).is_ok(), "asking again does nothing");
  assert_eq!(turn(root, now), TurnState::Joined);
}

#[tokio::test(flavor = "multi_thread")]
async fn while_answering_claude_takes_a_message_sent_now_into_the_answer() {
  send_now_joins_the_answer(AgentProvider::ClaudeCodeCli, "haiku").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn while_answering_codex_takes_a_message_sent_now_into_the_answer() {
  send_now_joins_the_answer(AgentProvider::CodexCli, "gpt-5.5").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn while_answering_codex_refusing_a_message_sent_now_leaves_it_for_a_turn_of_its_own() {
  let project = on(AgentProvider::CodexCli, "gpt-5.5");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let first = keep(root, chat, "Refuse a message.", &TurnState::Thinking);
  scheduler.answer_in_chat(first, tools()).await.expect("the turn starts");
  replied(root, &mut events, chat, 1).await;
  let now = keep(root, chat, "Also check the queue.", &TurnState::Steering);
  scheduler.steer_chat(now).expect("handed to the answer");
  let left = until(root, &mut events, now, |state| *state != TurnState::Steering).await;
  assert_eq!(left, TurnState::Queued(Queued::NotTaken), "Codex refused it");
  assert_eq!(
    until(root, &mut events, first, |state| !state.is_thinking()).await,
    TurnState::Answered
  );
  // Its own turn starts once the answer ended, as the app's follower starts it, and answers it.
  let mut db = ProjectDb::open(root).expect("open");
  assert_eq!(
    db.start_queued_chat_turn().expect("started"),
    Some(StartedQueued { chat, message: now })
  );
  scheduler.answer_in_chat(now, tools()).await.expect("its turn starts");
  assert_eq!(
    until(root, &mut events, now, |state| !state.is_thinking()).await,
    TurnState::Answered
  );
  assert_eq!(
    replies(root, chat).last().map(String::as_str),
    Some("Noted: Also check the queue.")
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn while_answering_a_message_the_answer_never_took_or_with_no_answer_under_way_waits_for_its_own_turn() {
  let project = on(AgentProvider::ClaudeCodeCli, "haiku");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  // No answer under way: it waits for a turn of its own at once.
  let idle = ChatId::generate();
  let alone = keep(root, idle, "Anyone there?", &TurnState::Steering);
  scheduler.steer_chat(alone).expect("left for its own turn");
  assert_eq!(turn(root, alone), TurnState::Queued(Queued::NotTaken));
  // An answer that reads nothing more of its input never takes it: once it ends, the message waits for its own turn.
  let chat = ChatId::generate();
  let first = keep(root, chat, "Keep thinking about the queue.", &TurnState::Thinking);
  scheduler.answer_in_chat(first, tools()).await.expect("the turn starts");
  replied(root, &mut events, chat, 1).await;
  let now = keep(root, chat, "Also check the queue.", &TurnState::Steering);
  scheduler.steer_chat(now).expect("handed to the answer");
  assert_eq!(turn(root, now), TurnState::Steering, "on its way while the answer runs");
  scheduler.stop_chat(chat).expect("stopped");
  assert_eq!(
    until(root, &mut events, first, |state| !state.is_thinking()).await,
    TurnState::Stopped
  );
  assert_eq!(
    until(root, &mut events, now, |state| *state != TurnState::Steering).await,
    TurnState::Queued(Queued::NotTaken)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn while_answering_queued_messages_wait_for_the_answer_and_stop_and_send_goes_first() {
  let project = on(AgentProvider::ClaudeCodeCli, "haiku");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let first = keep(root, chat, "Keep thinking about the queue.", &TurnState::Thinking);
  scheduler.answer_in_chat(first, tools()).await.expect("the turn starts");
  replied(root, &mut events, chat, 1).await;
  let after = keep(
    root,
    chat,
    "Then plan the release.",
    &TurnState::Queued(Queued::AfterAnswer),
  );
  let later = keep(root, chat, "And the notes.", &TurnState::Queued(Queued::AfterAnswer));
  let mut db = ProjectDb::open(root).expect("open");
  assert!(db.queued_chat_turn_ready().is_ok_and(|ready| !ready), "the answer runs");
  assert_eq!(
    db.start_queued_chat_turn().expect("read"),
    None,
    "nothing starts while it runs"
  );
  // Stop and send: the answer stops, and its message goes before those queued before it.
  let next = keep(root, chat, "Stop: what waits for me?", &TurnState::Queued(Queued::Next));
  scheduler.stop_chat(chat).expect("stopped");
  assert_eq!(
    until(root, &mut events, first, |state| !state.is_thinking()).await,
    TurnState::Stopped
  );
  for expected in [next, after, later] {
    assert_eq!(
      db.start_queued_chat_turn().expect("started"),
      Some(StartedQueued {
        chat,
        message: expected
      })
    );
    assert_eq!(db.start_queued_chat_turn().expect("read"), None, "one turn at a time");
    scheduler
      .answer_in_chat(expected, tools())
      .await
      .expect("its turn starts");
    assert_eq!(
      until(root, &mut events, expected, |state| !state.is_thinking()).await,
      TurnState::Answered
    );
  }
  let said = replies(root, chat);
  assert_eq!(
    &said[1..],
    [
      "Noted: Stop: what waits for me?",
      "Noted: Then plan the release.",
      "Noted: And the notes."
    ],
    "{said:?}"
  );
  // A queued message the person removes never starts.
  let removed = keep(root, chat, "Never mind.", &TurnState::Queued(Queued::AfterAnswer));
  assert!(db.remove_queued_chat_message(removed).expect("removed"));
  assert_eq!(db.start_queued_chat_turn().expect("read"), None);
  assert!(db.chat_turn(removed).expect("read").is_none());
}
