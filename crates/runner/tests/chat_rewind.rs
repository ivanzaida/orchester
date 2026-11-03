//! A rewound conversation of the Orchestrator chat at the engine boundary (ORC-333), end to end against
//! `orchester-fake-claude` and `orchester-fake-codex`, which answer `Noted: <prompt>`: after a person rewinds to before
//! a message, the next turn never resumes the session that heard it, and the new session is told only what was kept,
//! so the agent's answer shows it no longer knows the dropped message.
//! Run with `cargo test -p orchester-runner --features test-agent --test chat_rewind`.

mod support;

use std::{fs, path::Path};

use orchester_core::{
  connection::AgentProvider,
  conversation::MessageBody,
  id::{ChatId, ChatMessageId},
  orchestrator::{Chat, EntryKind, OrchestratorSettings, RewindPoint, TurnState},
  resource::ModelId,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_ipc::{ChatToken, ChatTools, Event};
use orchester_runner::execution::Scheduler;
use orchester_store::{ChatRewind, ProjectDb};
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

/// Answers `message` and waits until its turn ended.
async fn answer(
  (scheduler, events): (&Scheduler, &mut mpsc::UnboundedReceiver<Event>),
  root: &Path,
  message: ChatMessageId,
) -> TurnState {
  scheduler
    .answer_in_chat(message, tools())
    .await
    .expect("the turn starts");
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

/// Two turns of a conversation on `provider`, a rewind to before the second, and a third turn: how the CLI was
/// started for the third, and what it answered.
async fn after_a_rewind(provider: AgentProvider, model: &str) -> (String, String) {
  let project = on(provider, model);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let first = keep(root, chat, "Remember the word apple.");
  assert_eq!(
    answer((&scheduler, &mut events), root, first).await,
    TurnState::Answered
  );
  let second = keep(root, chat, "Now remember the word banana too.");
  assert_eq!(
    answer((&scheduler, &mut events), root, second).await,
    TurnState::Answered
  );
  let rewound = ProjectDb::open(root)
    .expect("open")
    .rewind_chat(chat, RewindPoint::Before(second), Timestamp::now())
    .expect("rewind");
  assert!(matches!(rewound, ChatRewind::Rewound { .. }), "{rewound:?}");
  let before = log(root).len();
  let third = keep(root, chat, "Which words do you know?");
  assert_eq!(
    answer((&scheduler, &mut events), root, third).await,
    TurnState::Answered
  );
  let started = log(root)[before..].to_owned();
  let replies = replies(root, chat);
  assert_eq!(replies.len(), 2, "the second answer was dropped: {replies:?}");
  (started, replies[1].clone())
}

/// What the new session was told, as `told` shows it: the kept messages, and not the dropped one.
fn knows_only_what_was_kept(told: &str) {
  assert!(told.contains("Remember the word apple."), "{told}");
  assert!(
    told.contains("Noted: Remember the word apple."),
    "its own kept answer too: {told}"
  );
  assert!(told.contains("Which words do you know?"), "{told}");
  assert!(!told.contains("banana"), "the dropped message never reaches it: {told}");
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_rewind_on_claude_starts_a_new_session_told_only_what_was_kept() {
  let (started, answer) = after_a_rewind(AgentProvider::ClaudeCodeCli, "haiku").await;
  assert!(
    started.contains(" --session-id ") && !started.contains("--resume"),
    "the session that heard the dropped message is not resumed: {started}"
  );
  // Claude Code's stand-in answers with the whole prompt it was given.
  knows_only_what_was_kept(&answer);
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_rewind_on_codex_starts_a_new_thread_told_only_what_was_kept() {
  let (started, answer) = after_a_rewind(AgentProvider::CodexCli, "gpt-5.5").await;
  assert!(
    started.contains("thread: new\n"),
    "the thread that heard the dropped message is not resumed: {started}"
  );
  // Codex's stand-in answers with the prompt's last line, and logs the whole prompt.
  assert_eq!(answer, "Noted: Which words do you know?");
  let told = started
    .lines()
    .find_map(|line| line.strip_prefix("prompt-words: "))
    .expect("the prompt is logged");
  knows_only_what_was_kept(told);
}
