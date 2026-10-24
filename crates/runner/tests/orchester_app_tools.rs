//! A run's started stand-in app is driven through both real per-invocation relays without asking.

mod support;

use orchester_core::{
  connection::AgentProvider,
  id::IdempotencyKey,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  run::RunStatus,
  store::ActivityStore,
};
use orchester_store::ProjectDb;
use support::{Setup, project_in, reported, scheduler_with, wait_for};

async fn exercise(provider: AgentProvider) {
  let mut permissions = AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    if provider == AgentProvider::CodexCli {
      CommandPermission::AnyInWorktree
    } else {
      CommandPermission::Allowed(AllowedCommands::default())
    },
    NetworkAccess::Off,
  );
  permissions.set_interactive_tool_approvals(true);
  let scratch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc23-run-app");
  std::fs::create_dir_all(&scratch).expect("scratch");
  let repo = tempfile::tempdir_in(scratch).expect("repo");
  let project = project_in(
    &Setup {
      provider,
      permissions: Some(permissions),
      ..Setup::new("Use the process tools. Call the run app.")
    },
    repo,
  );
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("start");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    run.status(),
    &RunStatus::Succeeded,
    "{:?}",
    reported(&db, id, "app-relay: ")
  );
  assert!(db.pending_tool_approvals(id).expect("approvals").is_empty());
  let reports = reported(&db, id, "app-relay: ");
  assert_eq!(
    reports,
    [
      "outside refused",
      "installed and other run refused",
      "pid refused",
      "app refused",
      "url refused",
      "navigate structured and redacted",
      "crowded .tmp and other layouts ignored",
      "screenshot saved",
      "errors redacted and invalid PNG refused",
      "redirect refused",
      "the program orchester called",
      "missing discovery refused",
      "duplicate discovery refused",
      "QA folder bound refused"
    ]
  );
  if provider == AgentProvider::ClaudeCodeCli {
    assert_eq!(reported(&db, id, "app-permission: ").len(), 15);
  }
  // The fake app echoes its token. Neither its answers nor the persisted transcript may contain it.
  let transcript = reported(&db, id, "");
  assert!(!transcript.join("\n").contains("stand-in-app-private-token"));
  for entry in db.activity(id, 0).expect("activity") {
    assert!(!format!("{:?}", entry.event).contains("stand-in-app-private-token"));
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn call_run_app_relay_codex() {
  exercise(AgentProvider::CodexCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn call_run_app_relay_claude() {
  exercise(AgentProvider::ClaudeCodeCli).await;
}
