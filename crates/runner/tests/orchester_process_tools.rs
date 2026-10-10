//! Process tools through each CLI's per-invocation relay, with MCP approvals enabled and a read-only step.

mod support;

use orchester_core::{
  connection::AgentProvider,
  id::IdempotencyKey,
  permission::{AgentPermissions, AllowedCommands, CommandPermission, NetworkAccess, RepositoryAccess},
  run::RunStatus,
};
use orchester_store::ProjectDb;
use support::{Setup, project, reported, scheduler_with, wait_for};

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
  let project = project(&Setup {
    provider,
    permissions: Some(permissions),
    ..Setup::new("Use the process tools.")
  });
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
    reported(&db, id, "orchester-call: ")
  );
  assert!(db.pending_tool_approvals(id).expect("approvals").is_empty());
  let starts = reported(&db, id, "process-stand-in: ");
  assert!(starts.iter().any(|line| line == "stopped"), "{starts:?}");
  let calls = reported(&db, id, "orchester-call: ");
  assert_eq!(calls.len(), 4, "{calls:?}");
  assert!(calls[0].starts_with("list_run_processes ok"), "{calls:?}");
  assert!(calls[0].contains("stand-in"), "{calls:?}");
  assert!(
    calls[1].starts_with("stop_run_process error") && calls[1].contains("refused"),
    "{calls:?}"
  );
  assert!(calls[2].starts_with("stop_run_process ok"), "{calls:?}");
  assert!(calls[3].contains("already gone"), "{calls:?}");
  if provider == AgentProvider::ClaudeCodeCli {
    assert!(
      reported(&db, id, "orchester-permission: ")
        .iter()
        .all(|line| line.ends_with("allow-by-rule"))
    );
  }
  // Listing includes both copies inside this worktree, then stopping the parent removes both.
  let worktree = root.join(".orchester/worktrees").join(id.to_string());
  let executor = orchester_exec::run_processes::RunProcesses::new(&worktree).expect("scope");
  let remaining = executor.run(orchester_core::orchester_tools::ProcessCall::List);
  assert!(!remaining.is_error(), "{}", remaining.text());
  assert_eq!(remaining.text(), "[]");
}

#[tokio::test(flavor = "multi_thread")]
async fn run_processes_relay_codex() {
  exercise(AgentProvider::CodexCli).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn run_processes_relay_claude() {
  exercise(AgentProvider::ClaudeCodeCli).await;
}
