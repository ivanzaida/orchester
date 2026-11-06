//! The context window of an Orchestrator conversation's session between turns (ORC-704), end to end against
//! `orchester-fake-claude` (`tests/support/fake_claude_chat.rs`): Claude Code reports a window only in a turn's result,
//! so a turn that resumes the session on the same model reads against the window the turn before reported, not the
//! table's estimate; a turn on another model never carries it. The fake's messages name no model and its results
//! report no window, so a reading without a carried window has none.
//! Run with `cargo test -p orchester-runner --features test-agent --test chat_context_window context_window`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::{AgentProvider, ConnectionAccount, context_use::ContextUse},
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, OrchestratorSettings, TurnState},
  resource::ModelId,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_ipc::{ChatToken, ChatTools, Event};
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, scheduler};

/// The tokens each message of the fake took in.
const FAKE_PROMPT: u64 = 12;
/// The window Claude Code reported for the session's earlier turn.
const REPORTED: u64 = 1_000_000;

/// A project whose Orchestrator runs on Claude Code with `haiku`.
fn on_haiku() -> TestProject {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Not used by the chat.")
  });
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  let connection = db.connections().expect("connections")[0].id();
  let model = ModelId::new("haiku").expect("model");
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

/// A conversation whose session's earlier turn on `model` reported [`REPORTED`], and its next message.
fn resumed_after_a_turn_on(root: &Path, model: &str) -> (ChatId, ChatMessageId) {
  let mut db = ProjectDb::open(root).expect("open");
  let chat = ChatId::generate();
  let body = MessageBody::new("What is running?").expect("text");
  let first = ChatMessageId::generate();
  db.keep_chat_message(
    &Chat::start(chat, &body, Timestamp::now()),
    (first, &body),
    Timestamp::now(),
  )
  .expect("kept");
  db.end_chat_turn(first, &TurnState::Answered, Timestamp::now())
    .expect("answered");
  let connection = db.connections().expect("connections")[0].id();
  let account = ConnectionAccount::Default
    .id(AgentProvider::ClaudeCodeCli)
    .expect("an account");
  db.set_chat_session(chat, "session-long", (connection, account))
    .expect("session");
  db.set_chat_context(chat, &ContextUse::reported(71_300, REPORTED))
    .expect("context");
  let model = ModelId::new(model).expect("model");
  db.set_chat_reported_window(chat, &model, REPORTED).expect("window");
  let kept = db.chat(chat).expect("read").expect("kept");
  let next = ChatMessageId::generate();
  let body = MessageBody::new("And what waits?").expect("text");
  db.keep_chat_message(&kept, (next, &body), Timestamp::now())
    .expect("kept");
  (chat, next)
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

fn context(root: &Path, chat: ChatId) -> Option<ContextUse> {
  ProjectDb::open(root).expect("open").chat_context(chat).expect("read")
}

#[tokio::test(flavor = "multi_thread")]
async fn context_window_a_resumed_turn_on_the_same_model_reads_against_the_reported_window() {
  let project = on_haiku();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let (chat, next) = resumed_after_a_turn_on(root, "haiku");
  scheduler.answer_in_chat(next, tools()).await.expect("the turn starts");
  assert_eq!(ended(root, &mut events, next).await, TurnState::Answered);
  assert_eq!(
    context(root, chat),
    Some(ContextUse::reported(FAKE_PROMPT, REPORTED)),
    "the readings before this turn's own window use the one the turn before reported, not an estimate"
  );
  let haiku = ModelId::new("haiku").expect("model");
  assert_eq!(
    ProjectDb::open(root)
      .expect("open")
      .chat_reported_window(chat, &haiku)
      .expect("read"),
    Some(REPORTED),
    "kept for the turn after"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn context_window_a_turn_on_another_model_does_not_carry_the_previous_models_window() {
  let project = on_haiku();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let (chat, next) = resumed_after_a_turn_on(root, "opus");
  scheduler.answer_in_chat(next, tools()).await.expect("the turn starts");
  assert_eq!(ended(root, &mut events, next).await, TurnState::Answered);
  assert_eq!(
    context(root, chat),
    Some(ContextUse::unknown_window(FAKE_PROMPT)),
    "opus's window is not haiku's: nothing reported one for this model"
  );
}
