//! Why auto mode's reviewer gave no verdict (ORC-587), end to end with `orchester-fake-claude` as the agent and as a
//! signed-out reviewer: the review keeps a short detail of why in Claude Code's own words, and the runner logs it at
//! warn with the run, the request, the connection and the model; neither holds the value of the project's variable the
//! reviewer ran with, nor its prompt, both of which its error repeats. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test reviewer_failure reviewer_failure`.

mod support;

use std::{
  collections::BTreeMap,
  io,
  sync::{Arc, Mutex},
};

use orchester_core::{
  connection::AgentProvider,
  id::IdempotencyKey,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess},
  resource::{CommandEnvironment, VariableName},
  run::{
    Answerer, ApprovalAnswer,
    review::{AutoModeSettings, DEFAULT_REVIEW_MODEL, Review, ReviewFailure},
  },
  store::SettingsStore,
};
use orchester_store::ProjectDb;

use self::support::{PATIENCE, Setup, project, scheduler_with, wait_for};

/// The value of a project variable the reviewer runs with.
const SECRET: &str = "orc587-a-value-nobody-may-read";

/// What the runner logs, kept for the test to read.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl io::Write for Captured {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.0.lock().expect("the log").extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

impl Captured {
  fn text(&self) -> String {
    String::from_utf8_lossy(&self.0.lock().expect("the log")).into_owned()
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn reviewer_failure_keeps_why_without_secrets_or_prompt_and_logs_it_at_warn() {
  let log = Captured::default();
  let writer = log.clone();
  tracing_subscriber::fmt()
    .with_writer(move || writer.clone())
    .with_ansi(false)
    .with_max_level(tracing::Level::WARN)
    .init();
  let listed = AllowedCommands::new(vec![CommandPrefix::new("git").expect("prefix")]).expect("list");
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    permissions: Some(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(listed),
      NetworkAccess::On,
    )),
    ..Setup::new("Run `cargo build`. Reviewer signed out.")
  });
  let root = project.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  db.save_auto_mode_settings(&AutoModeSettings { reviewer: None })
    .expect("the reviewer");
  let variable = VariableName::new("FAKE_REVIEW_SECRET").expect("name");
  let environment = CommandEnvironment::new(BTreeMap::from([(variable, SECRET.to_owned())])).expect("environment");
  db.save_project_environment(&environment).expect("project environment");
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");

  let (request, review) = tokio::time::timeout(PATIENCE, async {
    loop {
      let db = ProjectDb::open(root).expect("open");
      if let Some(request) = db.pending_tool_approvals(id).expect("requests").first()
        && let Some(review) = db.tool_approval_review(id, &request.id).expect("review")
      {
        return (request.clone(), review);
      }
      events.recv().await.expect("notification");
    }
  })
  .await
  .expect("the failed review leaves the request to the person");
  let Review::Failed { failure, model, detail } = &review else {
    panic!("{review:?}");
  };
  assert_eq!(*failure, ReviewFailure::CouldNotRun);
  assert_eq!(model.as_deref(), Some(DEFAULT_REVIEW_MODEL));
  let detail = detail.as_deref().expect("why the reviewer could not run");
  assert!(
    detail.starts_with("the reviewer could not run: the CLI exited with code 1: Not logged in · Please run /login"),
    "{detail}"
  );
  assert!(!detail.contains(SECRET), "{detail}");
  assert!(!detail.contains("Reviewer signed out"), "no prompt text: {detail}");
  assert!(!detail.contains("untrusted"), "no prompt text: {detail}");

  let logged = log.text();
  let line = logged
    .lines()
    .find(|line| line.contains("the reviewer gave no verdict"))
    .unwrap_or_else(|| panic!("the failed review is logged: {logged}"));
  assert!(line.contains("WARN"), "{line}");
  for part in [
    &format!("run={id}"),
    &format!("request={}", request.id),
    "connection=",
    DEFAULT_REVIEW_MODEL,
    "CouldNotRun",
    "Not logged in · Please run /login",
  ] {
    assert!(line.contains(part), "{part} in {line}");
  }
  assert!(!logged.contains(SECRET), "{logged}");
  assert!(!logged.contains("Reviewer signed out"), "{logged}");

  scheduler
    .decide_tool_approval(id, request.id, ApprovalAnswer::Deny, Answerer::Person)
    .await
    .expect("denied");
  wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
}
