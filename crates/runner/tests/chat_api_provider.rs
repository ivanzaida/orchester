//! The Orchestrator chat on a Claude Code connection that runs on an API provider (`docs/design/api-providers.md`),
//! end to end against `orchester-fake-claude` (`tests/support/fake_claude_chat.rs`): a turn starts the CLI with the
//! provider's base URL and the chosen model in its variables and the key in `ANTHROPIC_AUTH_TOKEN`, as a step does,
//! never on the CLI's own sign-in; the key reaches the process and nothing the chat keeps, and a key the keychain
//! does not give keeps the turn from starting, said by the secret's name.
//!
//! `cargo test -p orchester-runner --features test-agent --test chat_api_provider chat_api_provider`

mod support;

use std::{fs, path::Path};

use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiProvider, BaseUrl},
  conversation::MessageBody,
  id::{ChatId, ChatMessageId, SecretId},
  orchestrator::{Chat, EntryKind, OrchestratorSettings, TurnFailure, TurnState},
  resource::ModelId,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{ConnectionStore, SecretStore},
  time::Timestamp,
};
use orchester_ipc::{ChatToken, ChatTools, Event};
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, TestProject, project, scheduler_with};

/// A fake key; the fake CLI says it when asked, so what the chat keeps must show it redacted.
const KEY: &str = "sk-fake-provider-chat-9c4e2a17";
const MODEL: &str = "deepseek-flash";
const URL: &str = "https://api.deepseek.com/anthropic";

/// A project whose Orchestrator runs on its Claude Code connection, which runs on the API provider at [`URL`] with
/// [`MODEL`] typed for it; the key is in the keychain when `in_keychain`, else only the secret's record is in the
/// project.
fn on_provider(in_keychain: bool) -> TestProject {
  on_provider_of(AgentProvider::ClaudeCodeCli, in_keychain)
}

/// As [`on_provider`], on a connection of `cli`.
fn on_provider_of(cli: AgentProvider, in_keychain: bool) -> TestProject {
  let secret = Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("DEEPSEEK_KEY").expect("valid name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  );
  let key = secret.id();
  let secrets = if in_keychain {
    vec![(secret.clone(), KEY)]
  } else {
    Vec::new()
  };
  let project = project(&Setup {
    provider: cli,
    secrets,
    ..Setup::new("Not used by the chat.")
  });
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  if !in_keychain {
    db.create_secret(&secret).expect("the secret's record");
  }
  let stored = db.connections().expect("connections").remove(0);
  let mut connection = AgentConnection::try_from(stored).expect("an agent connection");
  let model = ModelId::new(MODEL).expect("valid model");
  let api = ApiProvider::new(BaseUrl::new(URL).expect("valid base URL"), key, vec![model.clone()]).expect("a model");
  connection.set_api_provider(Some(api)).expect("on Default");
  let id = connection.id();
  db.save_connection(&connection.into_connection()).expect("saved");
  db.publish_orchestrator(&OrchestratorSettings::new(id, model, None), Timestamp::now())
    .expect("settings");
  project
}

fn tools() -> ChatTools {
  ChatTools {
    url: "http://127.0.0.1:9/mcp".to_owned(),
    token: ChatToken::new("chat-token-provider".to_owned()),
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

/// Answers `text` in `chat` and waits for its turn to end.
async fn answered(
  project: &TestProject,
  events: &mut mpsc::UnboundedReceiver<Event>,
  scheduler: &orchester_runner::execution::Scheduler,
  (chat, text): (ChatId, &str),
) -> TurnState {
  let root = project.repo.path();
  let message = keep(root, chat, text);
  scheduler
    .answer_in_chat(message, tools())
    .await
    .expect("the turn is taken");
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

/// The value of the log's line that starts with `prefix`.
fn logged(log: &str, prefix: &str) -> Option<String> {
  log
    .lines()
    .find_map(|line| line.strip_prefix(prefix))
    .map(str::to_owned)
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_api_provider_turn_runs_on_the_providers_url_model_and_key() {
  let project = on_provider(true);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let chat = ChatId::generate();
  let state = answered(&project, &mut events, &scheduler, (chat, "What is running?")).await;
  assert_eq!(state, TurnState::Answered);
  assert_eq!(replies(root, chat), ["Noted: What is running?"]);
  let started = log(root);
  assert_eq!(
    logged(&started, "provider: ").as_deref(),
    Some(format!("{URL} {MODEL} {MODEL} {MODEL}").as_str()),
    "{started}"
  );
  assert_eq!(logged(&started, "provider key: ").as_deref(), Some("present"));
  let names = logged(&started, "anthropic-env: ").expect("the variables are logged");
  assert!(names.split(',').any(|name| name == "ANTHROPIC_AUTH_TOKEN"), "{names}");
  assert!(
    !names.split(',').any(|name| name == "ANTHROPIC_API_KEY"),
    "no other key reaches the provider: {names}"
  );
  assert!(
    logged(&started, "argv: ").is_some_and(|argv| argv.contains(&format!("--model {MODEL}"))),
    "{started}"
  );
  assert!(!started.contains(KEY), "the key is on no command line: {started}");
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_api_provider_codex_turn_runs_on_the_providers_url_and_key() {
  let project = on_provider_of(AgentProvider::CodexCli, true);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let chat = ChatId::generate();
  let state = answered(&project, &mut events, &scheduler, (chat, "What is running?")).await;
  assert_eq!(state, TurnState::Answered);
  assert_eq!(replies(root, chat), ["Noted: What is running?"]);
  let started = log(root);
  let argv = logged(&started, "codex-argv: ").expect("logged");
  assert!(argv.contains(r#"model_provider="orchester""#), "{argv}");
  assert!(
    argv.contains(&format!(r#"base_url="{URL}""#)) && argv.contains(r#"env_key="ORCHESTER_PROVIDER_API_KEY""#),
    "{argv}"
  );
  assert!(
    argv.contains(" -c features.shell_tool=false "),
    "still confined: {argv}"
  );
  assert!(!started.contains(KEY), "the key is on no command line: {started}");
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_api_provider_key_never_reaches_what_the_chat_keeps() {
  let project = on_provider(true);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let chat = ChatId::generate();
  let said = answered(&project, &mut events, &scheduler, (chat, "Say the key")).await;
  assert_eq!(said, TurnState::Answered);
  assert_eq!(replies(root, chat), ["The key is <redacted>."]);

  let failed = answered(&project, &mut events, &scheduler, (chat, "Fail with the key")).await;
  let TurnState::Failed(TurnFailure::Failed { reason }) = &failed else {
    panic!("the turn fails with the CLI's reason: {failed:?}");
  };
  assert!(reason.contains("<redacted>") && !reason.contains(KEY), "{reason}");
  let entries = ProjectDb::open(root)
    .expect("open")
    .chat_entries(chat)
    .expect("entries");
  assert!(
    !format!("{entries:?}").contains(KEY),
    "the transcript does not hold the key"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn chat_api_provider_no_turn_starts_without_the_key() {
  let project = on_provider(false);
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let chat = ChatId::generate();
  let state = answered(&project, &mut events, &scheduler, (chat, "What is running?")).await;
  let TurnState::Failed(TurnFailure::CannotStart { reason }) = &state else {
    panic!("the turn cannot start: {state:?}");
  };
  assert!(reason.contains("DEEPSEEK_KEY"), "the reason names the secret: {reason}");
  assert!(
    !root.join(".orchester").join("fake-chat.log").exists(),
    "the CLI never started, so never on its own sign-in"
  );
}
