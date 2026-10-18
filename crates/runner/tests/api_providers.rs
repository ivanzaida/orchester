//! Agent CLIs on API providers (`docs/design/api-providers.md`), end to end against the fake CLIs: Claude Code starts
//! with the provider's base URL and the step's model in its variables and the key in `ANTHROPIC_AUTH_TOKEN`, Codex
//! with a model provider of its own and the key in its `env_key`; the key reaches the process and nothing the run
//! keeps, and a key the keychain does not give keeps the agent from starting, said by the secret's name.
//!
//! `cargo test -p orchester-runner --features test-agent --test api_providers api_providers`

mod support;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, ApiProvider, BaseUrl, stream::AgentEvent},
  id::{IdempotencyKey, RunId, SecretId},
  resource::ModelId,
  run::RunStatus,
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  store::{ActivityStore, ConnectionStore, SecretStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use self::support::{Setup, TestProject, project, reported, scheduler_with, settled, threads, wait_for};

/// A fake key; the fake CLIs print it, so the activity must show it redacted.
const KEY: &str = "sk-fake-provider-runner-5d1e8b70";
const MODEL: &str = "deepseek-flash";

fn key_secret() -> Secret {
  Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new("DEEPSEEK_KEY").expect("valid name"),
      kind: SecretKind::ApiKey,
      environment: Environment::Development,
    },
    Timestamp::now(),
  )
}

/// A project whose Builder runs `provider`'s CLI on the API provider at `url`; the key is in the keychain when
/// `in_keychain`, else only the secret's record is in the project.
fn on_provider(provider: AgentProvider, url: &str, in_keychain: bool) -> TestProject {
  on_provider_with(provider, url, in_keychain, "Append one line.")
}

/// [`on_provider`] for a task with `brief`.
fn on_provider_with(provider: AgentProvider, url: &str, in_keychain: bool, brief: &'static str) -> TestProject {
  let secret = key_secret();
  let key = secret.id();
  let secrets = if in_keychain {
    vec![(secret.clone(), KEY)]
  } else {
    Vec::new()
  };
  let project = project(&Setup {
    provider,
    model: Some(MODEL),
    secrets,
    ..Setup::new(brief)
  });
  let mut db = ProjectDb::open(project.repo.path()).expect("open");
  if !in_keychain {
    db.create_secret(&secret).expect("the secret's record");
  }
  let stored = db.connections().expect("connections").remove(0);
  let mut connection = AgentConnection::try_from(stored).expect("an agent connection");
  let api = ApiProvider::new(
    BaseUrl::new(url).expect("valid base URL"),
    key,
    vec![ModelId::new(MODEL).expect("valid model")],
  )
  .expect("a model");
  connection.set_api_provider(Some(api)).expect("on Default");
  db.save_connection(&connection.into_connection()).expect("saved");
  project
}

/// Runs the project's task until it ends or waits for a person.
async fn ran(project: &TestProject) -> (ProjectDb, RunId, RunStatus) {
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, settled).await;
  (ProjectDb::open(root).expect("open"), id, run.status().clone())
}

/// Every event of the run's activity, as kept.
fn activity_text(db: &ProjectDb, run: RunId) -> String {
  db.activity(run, 0)
    .expect("activity")
    .into_iter()
    .map(|entry| format!("{:?}", entry.event))
    .collect::<Vec<_>>()
    .join("\n")
}

#[tokio::test(flavor = "multi_thread")]
async fn api_providers_claude_code_runs_on_the_providers_url_models_and_key() {
  let project = on_provider(AgentProvider::ClaudeCodeCli, "https://api.deepseek.com/anthropic", true);
  let (db, run, status) = ran(&project).await;
  assert_eq!(status, RunStatus::Succeeded);
  assert_eq!(
    reported(&db, run, "provider: "),
    [format!("https://api.deepseek.com/anthropic {MODEL} {MODEL} {MODEL}")]
  );
  assert_eq!(reported(&db, run, "provider key: "), ["<redacted>"]);
  let names = reported(&db, run, "env: ").join(",");
  assert!(names.contains("ANTHROPIC_AUTH_TOKEN"), "{names}");
  assert!(!names.split(',').any(|name| name == "ANTHROPIC_API_KEY"), "{names}");
  assert!(
    !activity_text(&db, run).contains(KEY),
    "the key is in nothing the run keeps"
  );
  assert!(
    db.invocation_accounts(run).expect("accounts").is_empty(),
    "a key's run counts against no account"
  );
  let invocation = db.invocations(run).expect("invocations")[0].id;
  let (base_url, _) = db
    .invocation_provider(invocation)
    .expect("read")
    .expect("the provider it started on is recorded");
  assert_eq!(base_url.as_str(), "https://api.deepseek.com/anthropic");
}

#[tokio::test(flavor = "multi_thread")]
async fn api_providers_codex_runs_on_a_model_provider_of_its_own() {
  let project = on_provider(AgentProvider::CodexCli, "https://api.deepseek.com", true);
  let (db, run, status) = ran(&project).await;
  assert_eq!(status, RunStatus::Succeeded);
  assert_eq!(
    reported(&db, run, "provider: "),
    [
      "model_providers.orchester={name=\"Orchester API provider\",base_url=\"https://api.deepseek.com\",\
      env_key=\"ORCHESTER_PROVIDER_API_KEY\",wire_api=\"responses\"}"
    ]
  );
  assert_eq!(reported(&db, run, "provider key: "), ["<redacted>"]);
  let started = threads(&db, run);
  assert_eq!(started[0]["modelProvider"], "orchester");
  assert_eq!(started[0]["model"], MODEL);
  assert!(
    !activity_text(&db, run).contains(KEY),
    "the key is in nothing the run keeps"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn api_providers_no_agent_starts_without_the_key() {
  let project = on_provider(
    AgentProvider::ClaudeCodeCli,
    "https://api.deepseek.com/anthropic",
    false,
  );
  let (db, run, status) = ran(&project).await;
  assert_ne!(status, RunStatus::Succeeded);
  let problems: Vec<String> = db
    .activity(run, 0)
    .expect("activity")
    .into_iter()
    .filter_map(|entry| match entry.event {
      AgentEvent::RunnerProblem { problem } => Some(format!("{problem:?}")),
      _ => None,
    })
    .collect();
  assert!(
    problems.iter().any(|problem| problem.contains("DEEPSEEK_KEY")),
    "the problem names the secret: {problems:?}"
  );
  assert!(reported(&db, run, "provider: ").is_empty(), "the CLI did not start");
}

#[tokio::test(flavor = "multi_thread")]
async fn api_providers_codex_on_another_model_provider_is_refused() {
  let project = on_provider(AgentProvider::CodexCli, "https://another-provider.example", true);
  let (db, run, status) = ran(&project).await;
  assert_ne!(status, RunStatus::Succeeded);
  let activity = activity_text(&db, run);
  assert!(
    activity.contains("another model provider than the connection's API provider"),
    "{activity}"
  );
  assert!(!activity.contains(KEY));
}

#[tokio::test(flavor = "multi_thread")]
async fn api_providers_codex_refuses_a_users_provider_of_the_same_id() {
  let project = on_provider(AgentProvider::CodexCli, "https://user-provider-clash.example", true);
  let (db, run, status) = ran(&project).await;
  assert_ne!(status, RunStatus::Succeeded);
  let activity = activity_text(&db, run);
  assert!(activity.contains("model provider named `orchester`"), "{activity}");
  assert!(threads(&db, run).is_empty(), "no thread starts");
}

/// Runs a Claude Code task on DeepSeek until it stops at the fake's plan limit, puts its connection on the provider at
/// `then` (with a new key secret when `new_key`), resumes it and answers the arguments of the invocation that resumed.
async fn resumed_after(then: &str, new_key: bool) -> String {
  let project = on_provider_with(
    AgentProvider::ClaudeCodeCli,
    "https://api.deepseek.com/anthropic",
    true,
    "Hit the plan limit",
  );
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let at_limit = |run: &orchester_core::run::Run| {
    matches!(
      run.status(),
      RunStatus::Paused(orchester_core::run::PauseReason::LimitReached(_))
    )
  };
  wait_for(root, &mut events, id, at_limit).await;
  let mut db = ProjectDb::open(root).expect("open");
  let mut connection = AgentConnection::try_from(db.connections().expect("connections").remove(0)).expect("CLI");
  let current = connection.api_provider().expect("on the provider").clone();
  let key = if new_key {
    let secret = Secret::new(
      SecretId::generate(),
      NewSecret {
        name: SecretName::new("OTHER_DEEPSEEK_KEY").expect("valid name"),
        kind: SecretKind::ApiKey,
        environment: Environment::Development,
      },
      Timestamp::now(),
    );
    db.create_secret(&secret).expect("another key secret");
    let value = orchester_core::secret::SecretValue::new(KEY.to_owned()).expect("not blank");
    orchester_core::secret::SecretVault::store(project.vault.as_ref(), &secret.vault_key(project.project), &value)
      .expect("its value");
    secret.id()
  } else {
    current.key()
  };
  let api = ApiProvider::new(
    BaseUrl::new(then).expect("valid"),
    key,
    vec![ModelId::new(MODEL).expect("valid")],
  )
  .expect("a model");
  connection.set_api_provider(Some(api)).expect("on Default");
  db.save_connection(&connection.into_connection()).expect("saved");
  drop(db);
  for _ in 0..50 {
    match scheduler.resume_run(id).await {
      Ok(()) => break,
      Err(_) => tokio::time::sleep(std::time::Duration::from_millis(100)).await,
    }
  }
  let db = ProjectDb::open(root).expect("open");
  for _ in 0..200 {
    if reported(&db, id, "argv: ").len() >= 2 {
      break;
    }
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  }
  reported(&db, id, "argv: ").pop().expect("a second invocation")
}

#[tokio::test(flavor = "multi_thread")]
async fn api_providers_a_session_continues_only_on_the_provider_and_key_it_was_made_on() {
  let same = resumed_after("https://api.deepseek.com/anthropic", false).await;
  assert!(
    same.contains("--resume"),
    "the same provider continues the session: {same}"
  );
  let host = resumed_after("https://gateway.example.com", false).await;
  assert!(!host.contains("--resume"), "another host starts a new session: {host}");
  let key = resumed_after("https://api.deepseek.com/anthropic", true).await;
  assert!(!key.contains("--resume"), "another key starts a new session: {key}");
}
