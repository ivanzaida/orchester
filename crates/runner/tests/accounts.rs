//! Agents run on their connection's account: the CLI starts with `CLAUDE_CONFIG_DIR` / `CODEX_HOME` set to the
//! account's folder, Default leaves the environment's, and no run starts without an account or its folder. End to
//! end against the fake CLIs. Run with `cargo test -p orchester-runner --features test-agent --test accounts`.

mod support;

use std::path::Path;

use orchester_core::{
  account::AccountCli,
  connection::{AccountStartProblem, AgentConnection, AgentProvider, ConnectionAccount, ConnectionCheck, LastCheck},
  id::{AgentAccountId, IdempotencyKey},
  resource::StepKey,
  run::{PreflightProblem, RunStatus},
  store::ConnectionStore,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;
use tempfile::TempDir;

use self::support::{Setup, TestProject, project, reported, scheduler, wait_for};

fn on(provider: AgentProvider) -> TestProject {
  project(&Setup {
    provider,
    ..Setup::new("Append one line.")
  })
}

/// Puts the project's connection on `account`, keeping its check as made on that account.
fn choose(root: &Path, account: ConnectionAccount) {
  let mut db = ProjectDb::open(root).expect("open");
  let stored = db.connections().expect("connections").remove(0);
  let check: Option<ConnectionCheck> = match stored.last_check() {
    Some(LastCheck::Agent(check)) => Some(check.clone()),
    _ => None,
  };
  let mut connection = AgentConnection::try_from(stored).expect("an agent connection");
  connection.set_account(account).expect("an agent CLI");
  if let Some(check) = check {
    connection.record_check(check).expect("its kind");
  }
  db.save_connection(&connection.into_connection()).expect("saved");
}

async fn folder_the_agent_saw(provider: AgentProvider, account: Option<&TempDir>) -> Vec<String> {
  let project = on(provider);
  let root = project.repo.path();
  if let Some(folder) = account {
    choose(
      root,
      ConnectionAccount::Account {
        id: AgentAccountId::generate(),
        folder: folder.path().to_path_buf(),
      },
    );
  }
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  reported(&ProjectDb::open(root).expect("open"), id, "account folder: ")
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_claude_code_runs_with_its_accounts_config_folder() {
  let folder = tempfile::tempdir().expect("an account folder");
  let seen = folder_the_agent_saw(AgentProvider::ClaudeCodeCli, Some(&folder)).await;
  assert_eq!(seen, [folder.path().display().to_string()]);
  assert_eq!(AccountCli::ClaudeCode.folder_variable(), "CLAUDE_CONFIG_DIR");
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_codex_runs_with_its_accounts_home() {
  let folder = tempfile::tempdir().expect("an account folder");
  let seen = folder_the_agent_saw(AgentProvider::CodexCli, Some(&folder)).await;
  assert_eq!(seen, [folder.path().display().to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_default_leaves_the_folder_as_the_environment_has_it() {
  let seen = folder_the_agent_saw(AgentProvider::ClaudeCodeCli, None).await;
  let inherited: Vec<String> = std::env::var_os("CLAUDE_CONFIG_DIR")
    .map(|folder| folder.to_string_lossy().into_owned())
    .into_iter()
    .collect();
  assert_eq!(seen, inherited);
}

#[tokio::test(flavor = "multi_thread")]
async fn accounts_no_run_starts_without_an_account_or_its_folder() {
  let project = on(AgentProvider::ClaudeCodeCli);
  let root = project.repo.path();
  let step = StepKey::new("implement").expect("key");
  choose(root, ConnectionAccount::Unassigned);
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::AccountUnavailable {
        step: step.clone(),
        reason: AccountStartProblem::NoAccount,
      }]
    })
  );
  let gone = root.join("no-such-account-folder");
  choose(
    root,
    ConnectionAccount::Account {
      id: AgentAccountId::generate(),
      folder: gone.clone(),
    },
  );
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::AccountUnavailable {
        step,
        reason: AccountStartProblem::FolderMissing { folder: gone },
      }]
    })
  );
}
