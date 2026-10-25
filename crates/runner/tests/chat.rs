//! The Orchestrator chat's turns, end to end against `orchester-fake-claude` (`tests/support/fake_claude_chat.rs`):
//! a message the desktop kept is answered by Claude Code started read-only in the project folder with only the
//! chat's domain tools, the next turn resumes its session, a turn can be stopped, and how a turn fails is kept. The
//! turns on Codex are in `chat_codex.rs`.
//! Run with `cargo test -p orchester-runner --features test-agent --test chat`.

mod support;

use std::{fs, num::NonZeroU32, path::Path};

use orchester_core::{
  connection::{AgentProvider, ConnectionAccount},
  conversation::MessageBody,
  id::{AgentAccountId, ChatId, ChatMessageId},
  limits::ProjectLimits,
  orchestrator::{Chat, ChatEvent, EntryKind, OrchestratorSettings, SessionChange, TurnFailure, TurnState},
  resource::ModelId,
  store::{ActivityStore, ConnectionStore, SettingsStore},
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
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
  db.publish_orchestrator(&OrchestratorSettings::new(connection, model, None), Timestamp::now())
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
      r#" --settings {"permissions":{"blockReadsOutsideWorkingDirectories":true}} --tools Read,Glob,Grep --allowedTools mcp__orchester --disallowedTools Read("#
    ),
    "{started}"
  );
  assert!(
    started.contains("/orchester-mcp/**) ")
      && started.contains(" --permission-mode default --max-turns 30 --session-id "),
    "the runner's private files are never read: {started}"
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
async fn chat_starts_a_new_session_when_its_connection_moved_to_another_account() {
  let project = claude();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let message = keep(root, chat, "What is running?");
  let mut db = ProjectDb::open(root).expect("open");
  let connection = db.connections().expect("connections")[0].id();
  // The conversation's session was kept while the connection ran on another account: its CLI folder holds it.
  db.set_chat_session(chat, "session-elsewhere", (connection, AgentAccountId::generate()))
    .expect("session");
  scheduler
    .answer_in_chat(message, tools())
    .await
    .expect("the turn starts");
  assert_eq!(ended(root, &mut events, message).await, TurnState::Answered);
  let started = log(root);
  assert!(
    started.contains(" --session-id ") && !started.contains("--resume"),
    "a new session: {started}"
  );
  let db = ProjectDb::open(root).expect("open");
  let session = db.chat(chat).expect("read").expect("kept");
  let session = session.session().expect("the new session is kept");
  assert_ne!(session.id, "session-elsewhere");
  assert_eq!(
    Some(session.account),
    ConnectionAccount::Default.id(AgentProvider::ClaudeCodeCli),
    "with the account it lives on"
  );
  let said: Vec<EntryKind> = db
    .chat_entries(chat)
    .expect("entries")
    .into_iter()
    .map(|entry| entry.kind)
    .filter(|kind| matches!(kind, EntryKind::Event(_)))
    .collect();
  assert_eq!(
    said,
    [EntryKind::Event(ChatEvent::NewSession {
      because: SessionChange::Account
    })],
    "the conversation says the agent starts afresh"
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
  scheduler.stop_chat(chat).expect("stopped");
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
    (
      None,
      orchester_runner::execution::QaSlots::in_folder(&root.join(".orchester").join("qa-slots")),
    ),
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

#[tokio::test(flavor = "multi_thread")]
async fn a_turn_no_runner_answers_is_interrupted_while_another_runs_and_stop_ends_it() {
  let project = claude();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let thinking_chat = ChatId::generate();
  let thinking = keep(root, thinking_chat, "Keep thinking about the queue.");
  scheduler
    .answer_in_chat(thinking, tools())
    .await
    .expect("the turn starts");
  let wait = async {
    while replies(root, thinking_chat).is_empty() {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, wait).await.expect("the reply starts");
  // Kept, but the runner never got it (its answer was lost): no runner answers it.
  let (left, stopped) = (ChatId::generate(), ChatId::generate());
  let left_message = keep(root, left, "Hello?");
  let stopped_message = keep(root, stopped, "Hello again?");
  scheduler
    .attach(
      AttachSession::Fresh {
        previous_end: SessionEnd::Clean,
      },
      NonZeroU32::new(2).expect("above zero"),
    )
    .await
    .expect("attached");
  let turn = |message| {
    ProjectDb::open(root)
      .expect("open")
      .chat_turn(message)
      .expect("read")
      .expect("kept")
      .turn
  };
  assert_eq!(
    turn(left_message),
    TurnState::Failed(TurnFailure::Interrupted),
    "a desktop attaching ends the turn no runner answers"
  );
  assert!(turn(thinking).is_thinking(), "the turn under way goes on");

  // Kept after the attach and never answered: Stop ends it, since nothing else would.
  let later = keep(root, stopped, "Still there?");
  scheduler.stop_chat(stopped).expect("stopped");
  assert_eq!(turn(later), TurnState::Failed(TurnFailure::Interrupted));
  assert_eq!(turn(stopped_message), TurnState::Failed(TurnFailure::Interrupted));
  assert!(turn(thinking).is_thinking());
  scheduler.stop_chat(thinking_chat).expect("stopped");
  assert_eq!(ended(root, &mut events, thinking).await, TurnState::Stopped);
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_tokens_count_toward_the_daily_budget_and_a_used_one_refuses_a_turn() {
  let project = claude();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let chat = ChatId::generate();
  let first = keep(root, chat, "What is running?");
  scheduler.answer_in_chat(first, tools()).await.expect("the turn starts");
  assert_eq!(ended(root, &mut events, first).await, TurnState::Answered);
  let mut db = ProjectDb::open(root).expect("open");
  let spent = db.chat_turn_tokens(first).expect("read");
  assert!(spent > TokensUsed::ZERO, "the turn's usage is kept");
  let today = db.tokens_used_since(Timestamp::now().local_day_start()).expect("read");
  assert_eq!(today, spent, "and counted with today's usage");

  // Today's budget is used: the next message is refused as a run would be, and nothing starts.
  let tight = ProjectLimits::new(None, TokenBudget::new(1).expect("above zero"), None).expect("limits");
  db.save_project_limits(&tight).expect("save limits");
  let second = keep(root, chat, "And what waits?");
  assert_eq!(
    scheduler.answer_in_chat(second, tools()).await,
    Err(Rejection::DailyBudgetUsed {
      budget: tight.daily_budget()
    })
  );
  assert!(
    ProjectDb::open(root)
      .expect("open")
      .chat_turn(second)
      .expect("read")
      .expect("kept")
      .turn
      .is_thinking(),
    "no turn ran: the desktop forgets the message"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_turn_cut_at_the_time_limit_or_stopped_still_counts_what_it_spent() {
  let project = claude();
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  scheduler.limit_chat_turns(std::time::Duration::from_secs(2));
  let chat = ChatId::generate();
  let cut = keep(root, chat, "Keep thinking about the queue.");
  scheduler.answer_in_chat(cut, tools()).await.expect("the turn starts");
  let ended_as = ended(root, &mut events, cut).await;
  assert!(
    matches!(ended_as, TurnState::Failed(TurnFailure::TimeLimit { .. })),
    "{ended_as:?}"
  );
  let db = ProjectDb::open(root).expect("open");
  assert!(
    db.chat_turn_tokens(cut).expect("read") > TokensUsed::ZERO,
    "what it spent before the limit counts"
  );

  scheduler.limit_chat_turns(std::time::Duration::from_secs(600));
  let stopped = keep(root, chat, "Keep thinking once more.");
  scheduler
    .answer_in_chat(stopped, tools())
    .await
    .expect("the turn starts");
  let wait = async {
    while replies(root, chat).len() < 2 {
      events.recv().await.expect("the scheduler notifies");
    }
  };
  tokio::time::timeout(PATIENCE, wait).await.expect("the reply starts");
  scheduler.stop_chat(chat).expect("stopped");
  assert_eq!(ended(root, &mut events, stopped).await, TurnState::Stopped);
  assert!(
    db.chat_turn_tokens(stopped).expect("read") > TokensUsed::ZERO,
    "what a stopped turn spent counts"
  );
}
