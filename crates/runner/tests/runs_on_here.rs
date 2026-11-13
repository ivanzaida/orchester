//! A local run's agents start on what this computer chose for their profile (`docs/design/agent-accounts.md`,
//! "Agents run on accounts", decision 4): the connection picked here, not the one the profile's version names, and
//! the run is checked on that one before it starts. End to end against the fake CLI. Run with
//! `cargo test -p orchester-runner --features test-agent --test runs_on_here runs_on_here`.

mod support;

use std::path::Path;

use orchester_core::{
  connection::{AccountStartProblem, AgentConnection, AgentProvider, ConnectionAccount, LastCheck},
  id::{AgentAccountId, ConnectionId, IdempotencyKey, ResourceId},
  name::Name,
  resource::{ResourceKind, StepKey},
  run::{PreflightProblem, RunStatus},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::{ConnectionPick, ProjectDb};

use self::support::{Setup, TestProject, project, reported, scheduler, wait_for};

fn on_claude() -> TestProject {
  project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Append one line.")
  })
}

/// The project's Builder profile and the connection its version names.
fn builder(root: &Path) -> (ResourceId, AgentConnection) {
  let db = ProjectDb::open(root).expect("open");
  let profile = db
    .resources(ResourceKind::AgentProfile)
    .expect("profiles")
    .remove(0)
    .id();
  let own = db.connections().expect("connections").remove(0);
  (profile, AgentConnection::try_from(own).expect("an agent connection"))
}

/// Another connection to the program of `own`, checked as `own` was, called `called`, on an account in `folder`.
fn connection_on(root: &Path, own: &AgentConnection, called: &str, folder: &Path) -> ConnectionId {
  let account = ConnectionAccount::Account {
    id: AgentAccountId::generate(),
    folder: folder.to_path_buf(),
  };
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(called).expect("a name"),
    own.provider(),
    own.executable().expect("a program").to_path_buf(),
    Timestamp::now(),
  )
  .expect("a connection");
  connection.set_account(account).expect("an agent CLI");
  if let Some(check) = own.last_check() {
    connection.record_check(check.clone()).expect("its kind");
  }
  ProjectDb::open(root)
    .expect("open")
    .create_connection(&connection)
    .expect("stored");
  connection.id()
}

fn pick(root: &Path, profile: ResourceId, connection: ConnectionId) {
  ProjectDb::open(root)
    .expect("open")
    .set_connection_pick(
      profile,
      ConnectionPick {
        connection,
        account: None,
      },
    )
    .expect("picked");
}

/// Puts `own` on `account`, keeping its check.
fn move_own(root: &Path, own: &AgentConnection, account: ConnectionAccount) {
  let check = own.last_check().cloned();
  let mut moved = own.clone();
  moved.set_account(account).expect("an agent CLI");
  if let Some(check) = check {
    moved.record_check(check).expect("its kind");
  }
  ProjectDb::open(root)
    .expect("open")
    .save_connection(&moved.into_connection())
    .expect("saved");
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_on_here_the_agent_starts_on_the_connection_picked_for_its_profile() {
  let project = on_claude();
  let root = project.repo.path();
  let (profile, own) = builder(root);
  let folder = tempfile::tempdir().expect("an account folder");
  let picked = connection_on(root, &own, "Claude Code · Work", folder.path());
  pick(root, profile, picked);

  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    reported(&db, id, "account folder: "),
    [folder.path().display().to_string()],
    "the CLI started in the picked connection's account folder"
  );
  let step = StepKey::new("implement").expect("key");
  let ran_on = db.step_agent_connection(&run, &step).expect("read").expect("an agent");
  assert_eq!(ran_on.id(), picked);
  assert!(
    matches!(
      db.connection(own.id())
        .expect("read")
        .and_then(|own| own.last_check().cloned()),
      Some(LastCheck::Agent(_))
    ),
    "the profile's own connection is untouched"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_on_here_a_run_is_checked_on_the_picked_connection() {
  let project = on_claude();
  let root = project.repo.path();
  let (profile, own) = builder(root);
  let gone = root.join("no-such-account-folder");
  let picked = connection_on(root, &own, "Claude Code · Work", &gone);
  pick(root, profile, picked);
  let (scheduler, mut events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), project.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::AccountUnavailable {
        step: StepKey::new("implement").expect("key"),
        reason: AccountStartProblem::FolderMissing { folder: gone },
      }]
    }),
    "the profile's own connection could run it, but it does not run on that one here"
  );

  // The other way round: its own connection has no account, and the one picked here runs it.
  let folder = tempfile::tempdir().expect("an account folder");
  let usable = connection_on(root, &own, "Claude Code · Personal", folder.path());
  pick(root, profile, usable);
  move_own(root, &own, ConnectionAccount::Unassigned);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts on the picked connection");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
}
