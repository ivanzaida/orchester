//! The Orchestrator chat's turns, end to end against `orchester-fake-claude` (`tests/support/fake_claude_chat.rs`):
//! a message the desktop kept is answered by Claude Code started read-only in the project folder with only the
//! chat's domain tools, the next turn resumes its session, a turn can be stopped, and how a turn fails is kept.
//! Run with `cargo test -p orchester-runner --features test-agent --test chat`.

mod support;

use std::{fs, num::NonZeroU32, path::Path};

use orchester_core::{
  connection::AgentProvider,
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, EntryKind, OrchestratorSettings, TurnFailure, TurnState},
  resource::ModelId,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_ipc::{AttachSession, ChatToken, ChatTools, Event, Rejection, SessionEnd};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, scheduler};

const TOKEN: &str = "chat-token-7f3a9c";

fn claude() -> TestProject {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Not used by the chat.")
  });
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  let connection = db.connections().expect("connections")[0].id();
  let model = ModelId::new("haiku").expect("model");
  db.save_orchestrator_settings(&OrchestratorSettings::new(connection, model, None))
    .expect("settings");
  project
}

fn tools() -> ChatTools {
  ChatTools {
    url: "http://127.0.0.1:9/mcp".to_owned(),
    token: ChatToken::new(TOKEN.to_owned()),
  }
}

/// Keeps `text` as a new message of `chat` (started with it when new).
fn keep(root: &Path, chat: ChatId, text: &str) -> ChatMessageId {
  let mut db = ProjectDb::open(root).expect("open");
  let body = MessageBody::new(text).expect("text");
  let kept = db
    .chat(chat)
    .expect("chat")
    .unwrap_or_else(|| Chat::start(chat, &body, Timestamp::now()));
  let message = ChatMessageId::generate();
  assert!(
    db.keep_chat_message(&kept, (message, &body), Timestamp::now())
      .expect("kept")
  );
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

fn log(root: &Path) -> String {
  fs::read_to_string(root.join(".orchester").join("fake-chat.log")).expect("the fake logged its start")
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_answers_read_only_with_only_its_tools_and_resumes_its_session() {
  let project = claude();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let first = keep(root, chat, "What is running?");
  scheduler.answer_in_chat(first, tools()).await.expect("the turn starts");
  assert_eq!(ended(root, &mut events, first).await, TurnState::Answered);
  assert_eq!(replies(root, chat), ["Noted: What is running?"]);
  let session = ProjectDb::open(root)
    .expect("open")
    .chat(chat)
    .expect("read")
    .expect("kept");
  let session = session.session().expect("the session is kept").id.clone();

  let started = log(root);
  assert!(
    started.contains("--strict-mcp-config --setting-sources  --disable-slash-commands --system-prompt-file "),
    "{started}"
  );
  assert!(
    started.contains(
      " --tools Read,Glob,Grep --allowedTools Read Glob Grep mcp__orchester --permission-mode default --session-id "
    ),
    "{started}"
  );
  assert!(
    !started.contains("--json-schema"),
    "no step result is asked for: {started}"
  );
  assert!(
    started.contains("system-prompt: You are the Orchestrator of the project \"Kontur\""),
    "{started}"
  );
  assert!(
    started.contains("mcp-url: http://127.0.0.1:9/mcp\nmcp-token: present"),
    "{started}"
  );
  assert!(
    !started.contains(TOKEN),
    "the token stays in the configuration file: {started}"
  );
  let leftovers: Vec<_> = fs::read_dir(orchester_runner::private_file::directory(project.project))
    .map(|files| files.filter_map(Result::ok).map(|file| file.file_name()).collect())
    .unwrap_or_default();
  assert!(
    leftovers
      .iter()
      .all(|file| !file.to_string_lossy().starts_with("chat-")),
    "the turn's files are removed: {leftovers:?}"
  );

  let second = keep(root, chat, "And what waits?");
  scheduler
    .answer_in_chat(second, tools())
    .await
    .expect("the turn starts");
  assert_eq!(ended(root, &mut events, second).await, TurnState::Answered);
  assert!(
    log(root).contains(&format!("--resume {session}")),
    "the second turn resumes the session"
  );
  assert_eq!(
    replies(root, chat),
    ["Noted: What is running?", "Noted: And what waits?"]
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stopped_turn_keeps_what_it_said_and_a_failed_one_says_why() {
  let project = claude();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let thinking = keep(root, chat, "Keep thinking about the queue.");
  scheduler
    .answer_in_chat(thinking, tools())
    .await
    .expect("the turn starts");
  let wait = async {
    while replies(root, chat).is_empty() {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, wait).await.expect("the reply starts");
  let busy = keep(root, chat, "Another one.");
  assert_eq!(
    scheduler.answer_in_chat(busy, tools()).await,
    Err(Rejection::ChatBusy),
    "one message at a time"
  );
  assert!(
    scheduler.answer_in_chat(thinking, tools()).await.is_ok(),
    "asking again is no second turn"
  );
  assert!(!scheduler.is_idle(), "a turn under way is work");
  scheduler.stop_chat(chat);
  assert_eq!(ended(root, &mut events, thinking).await, TurnState::Stopped);
  assert_eq!(replies(root, chat), ["Let me look at the queue."]);

  let other = ChatId::generate();
  let signed_out = keep(root, other, "Please sign in first.");
  scheduler
    .answer_in_chat(signed_out, tools())
    .await
    .expect("the turn starts");
  assert_eq!(
    ended(root, &mut events, signed_out).await,
    TurnState::Failed(TurnFailure::NotSignedIn)
  );
  let failing = keep(root, other, "Fail now, please.");
  scheduler
    .answer_in_chat(failing, tools())
    .await
    .expect("the turn starts");
  assert_eq!(
    ended(root, &mut events, failing).await,
    TurnState::Failed(TurnFailure::Failed {
      reason: "Something broke on the provider's side.".to_owned()
    })
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_turn_needs_a_kept_message_and_an_orchestrator_and_a_new_runner_ends_stale_ones() {
  let project = support::project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Not used by the chat.")
  });
  let root = project.repo.path();
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.answer_in_chat(ChatMessageId::generate(), tools()).await,
    Err(Rejection::ChatMessageNotFound)
  );
  let chat = ChatId::generate();
  let message = keep(root, chat, "Hello?");
  assert_eq!(
    scheduler.answer_in_chat(message, tools()).await,
    Err(Rejection::NoOrchestrator)
  );

  let fresh = Scheduler::new(
    root,
    orchester_runner::notify::Notifier::new(mpsc::unbounded_channel().0),
    std::sync::Arc::new(support::vault::MemoryVault::default()),
    None,
  );
  fresh
    .attach(
      AttachSession::Fresh {
        previous_end: SessionEnd::Clean,
      },
      NonZeroU32::new(2).expect("above zero"),
    )
    .await
    .expect("attached");
  let turn = ProjectDb::open(root)
    .expect("open")
    .chat_turn(message)
    .expect("read")
    .expect("kept")
    .turn;
  assert_eq!(turn, TurnState::Failed(TurnFailure::Interrupted));
}
