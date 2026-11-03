//! A fork of a conversation of the Orchestrator chat at the engine boundary (ORC-334), end to end against
//! `orchester-fake-claude` and `orchester-fake-codex`, which answer `Noted: <prompt>`: after a person forks a
//! conversation from before its second message, the fork's first turn starts a session of its own, never the
//! original's, told only the copied conversation, so the agent goes on from that history without the second message;
//! the original's next turn resumes its own session.
//! Run with `cargo test -p orchester-runner --features test-agent --test chat_fork`.

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
use orchester_store::{ChatFork, ProjectDb};
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

/// How the CLI was started for a turn, and what it answered.
struct Turned {
  started: String,
  answer: String,
}

/// Keeps `text` in `chat` and answers it: how the CLI was started for it, and its answer.
async fn turn(
  (scheduler, events): (&Scheduler, &mut mpsc::UnboundedReceiver<Event>),
  root: &Path,
  (chat, text): (ChatId, &str),
) -> Turned {
  let before = log(root).len();
  let message = keep(root, chat, text);
  assert_eq!(answer((scheduler, events), root, message).await, TurnState::Answered);
  Turned {
    started: log(root)[before..].to_owned(),
    answer: replies(root, chat).pop().expect("an answer"),
  }
}

/// Two turns of a conversation on `provider`, a fork from before the second, a turn in the fork and one in the
/// original.
async fn after_a_fork(provider: AgentProvider, model: &str) -> (Turned, Turned) {
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
  let fork = ChatId::generate();
  let forked = ProjectDb::open(root)
    .expect("open")
    .fork_chat(
      (chat, RewindPoint::Before(second)),
      (fork, "Apples · fork"),
      Timestamp::now(),
    )
    .expect("fork");
  assert!(matches!(forked, ChatFork::Forked { .. }), "{forked:?}");
  let in_fork = turn((&scheduler, &mut events), root, (fork, "Which words do you know?")).await;
  let in_original = turn((&scheduler, &mut events), root, (chat, "And which now?")).await;
  assert_eq!(replies(root, chat).len(), 3, "the original kept all its answers");
  (in_fork, in_original)
}

/// What the fork's new session was told, as `told` shows it: the copied messages, and not the one after the point.
fn knows_only_what_was_copied(told: &str) {
  assert!(told.contains("Remember the word apple."), "{told}");
  assert!(
    told.contains("Noted: Remember the word apple."),
    "its own copied answer too: {told}"
  );
  assert!(told.contains("Which words do you know?"), "{told}");
  assert!(
    !told.contains("banana"),
    "the message after the point never reaches it: {told}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_fork_on_claude_starts_a_session_of_its_own_told_only_what_was_copied() {
  let (fork, original) = after_a_fork(AgentProvider::ClaudeCodeCli, "haiku").await;
  assert!(
    fork.started.contains(" --session-id ") && !fork.started.contains("--resume"),
    "the fork never resumes the original's session: {}",
    fork.started
  );
  // Claude Code's stand-in answers with the whole prompt it was given.
  knows_only_what_was_copied(&fork.answer);
  assert!(
    original.started.contains("--resume"),
    "the original goes on in its own session: {}",
    original.started
  );
  assert_eq!(original.answer, "Noted: And which now?");
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_fork_on_codex_starts_a_thread_of_its_own_told_only_what_was_copied() {
  let (fork, original) = after_a_fork(AgentProvider::CodexCli, "gpt-5.5").await;
  assert!(
    fork.started.contains("thread: new\n"),
    "the fork never resumes the original's thread: {}",
    fork.started
  );
  // Codex's stand-in answers with the prompt's last line, and logs the whole prompt.
  assert_eq!(fork.answer, "Noted: Which words do you know?");
  let told = fork
    .started
    .lines()
    .find_map(|line| line.strip_prefix("prompt-words: "))
    .expect("the prompt is logged");
  knows_only_what_was_copied(told);
  assert!(
    !original.started.contains("thread: new\n"),
    "the original goes on in its own thread: {}",
    original.started
  );
}
