//! The desktop's runner handling against the real `orchester-runner` program.

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::Duration,
};

use orchester_core::id::{IdempotencyKey, TaskId};
use orchester_desktop::{
  activity::Activity,
  app_paths::runner_beside,
  close::{CloseKind, session_end},
  runner::{
    QUIET_STOP_TIMEOUT, RequestError, RunRequestError, RunnerStatus, Runners, ShutdownOutcome,
    state::{RunnerFailure, RunnerState, reconnect_delay},
  },
  workspace::OpenProject,
};
use orchester_ipc::{Rejection, SessionEnd::Clean};
use orchester_store::SessionEnd;
use tempfile::TempDir;
use tokio::{runtime::Handle, sync::watch, time::timeout};

const PATIENCE: Duration = Duration::from_secs(20);

/// Cargo exposes `CARGO_BIN_EXE_*` only for binaries of the tested package, so the runner is
/// built into this profile's directory, where `cargo run` also puts `orchester.exe`.
fn runner_program() -> PathBuf {
  let status = Command::new(env!("CARGO"))
    .args(["build", "--quiet", "-p", "orchester-runner"])
    .status()
    .expect("run cargo");
  assert!(status.success(), "cargo build -p orchester-runner: {status}");
  let test_exe = std::env::current_exe().expect("test executable path");
  let profile_dir = test_exe
    .parent()
    .and_then(Path::parent)
    .expect("test executables live in <target>/<profile>/deps");
  runner_beside(&profile_dir.join("orchester.exe"))
}

/// `root` as an open project named Kontur.
fn open(root: &Path) -> OpenProject {
  OpenProject {
    project: orchester_core::project::Project::new(
      orchester_core::id::ProjectId::generate(),
      orchester_core::project::ProjectKey::new("ORC").expect("key"),
      orchester_core::name::Name::new("Kontur").expect("name"),
    ),
    repo_root: root.to_path_buf(),
  }
}

/// An initialized project: the runner reads its database when the desktop attaches.
fn project() -> TempDir {
  let project = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(project.path().join(".git")).expect("create .git");
  drop(orchester_store::ProjectDb::create(project.path(), open(project.path()).project).expect("create project"));
  project
}

async fn wait_for(
  updates: &mut watch::Receiver<RunnerStatus>,
  project: &Path,
  done: impl Fn(&RunnerState) -> bool,
) -> RunnerState {
  timeout(PATIENCE, async {
    loop {
      let state = updates.borrow_and_update().state_for(project);
      if done(&state) {
        return state;
      }
      updates.changed().await.expect("the runners outlive the test");
    }
  })
  .await
  .expect("runner state in time")
}

#[tokio::test(flavor = "multi_thread")]
async fn runner_process_goes_online_and_stops_gracefully() {
  let project = project();
  let runners = Runners::new(runner_program(), Handle::current(), Activity::new(), Clean);
  let mut updates = runners.subscribe();

  runners.show(Some(&open(project.path())));
  let online = wait_for(&mut updates, project.path(), |state| *state != RunnerState::Starting).await;
  assert_eq!(online, RunnerState::Online);

  let outcome = runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await;
  assert_eq!(outcome, ShutdownOutcome::Stopped);
  assert_eq!(runners.status().state(), &RunnerState::Stopped);
  assert_eq!(session_end(CloseKind::Quiet, outcome), SessionEnd::Clean);
}

#[tokio::test(flavor = "multi_thread")]
async fn runner_process_missing_program_fails_visibly() {
  let project = project();
  let missing = project.path().join("orchester-runner.exe");
  let runners = Runners::new(missing.clone(), Handle::current(), Activity::new(), Clean);
  let mut updates = runners.subscribe();

  runners.show(Some(&open(project.path())));
  let failed = wait_for(&mut updates, project.path(), |state| *state != RunnerState::Starting).await;
  assert_eq!(
    failed,
    RunnerState::Failed {
      cause: RunnerFailure::MissingExecutable { path: missing },
      attempts: 0
    }
  );
  assert_eq!(
    runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await,
    ShutdownOutcome::NoRunner
  );
}

/// A request queued while the runner's launch fails was never sent: it is answered as not connected,
/// not as lost. On one thread the request is queued before the task that launches the runner first
/// runs; which of the two that task handles first varies, so it is tried several times.
#[tokio::test]
async fn runner_process_request_queued_while_the_launch_fails_is_not_connected() {
  let project = project();
  let missing = project.path().join("orchester-runner.exe");
  for _ in 0..16 {
    let runners = Runners::new(missing.clone(), Handle::current(), Activity::new(), Clean);
    runners.show(Some(&open(project.path())));
    let answer = timeout(
      PATIENCE,
      runners.start_run(IdempotencyKey::generate(), TaskId::generate()),
    )
    .await
    .expect("answer in time");
    assert_eq!(answer, Err(RunRequestError::Request(RequestError::NotConnected)));
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn runner_process_answers_start_run_requests() {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = orchester_core::project::Project::new(
    orchester_core::id::ProjectId::generate(),
    orchester_core::project::ProjectKey::new("ORC").expect("key"),
    orchester_core::name::Name::new("Kontur").expect("name"),
  );
  drop(orchester_store::ProjectDb::create(repo.path(), project).expect("create project"));
  let runners = Runners::new(runner_program(), Handle::current(), Activity::new(), Clean);
  let (key, task) = (IdempotencyKey::generate(), TaskId::generate());
  assert_eq!(
    runners.start_run(key, task).await,
    Err(RunRequestError::Request(RequestError::NotConnected))
  );
  let mut updates = runners.subscribe();
  runners.show(Some(&open(repo.path())));
  wait_for(&mut updates, repo.path(), |state| *state == RunnerState::Online).await;

  let answer = timeout(PATIENCE, runners.start_run(key, task))
    .await
    .expect("answer in time");
  assert_eq!(answer, Err(RunRequestError::Rejected(Rejection::TaskNotFound)));
  runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn runner_process_keeps_the_previous_project_running_in_the_background() {
  let (first, second) = (project(), project());
  let runners = Runners::new(runner_program(), Handle::current(), Activity::new(), Clean);
  let mut updates = runners.subscribe();
  runners.show(Some(&open(first.path())));
  wait_for(&mut updates, first.path(), |state| *state == RunnerState::Online).await;
  runners.show(Some(&open(second.path())));
  wait_for(&mut updates, second.path(), |state| *state == RunnerState::Online).await;

  // Showing the first project again shows its runner as it is: it kept running.
  runners.show(Some(&open(first.path())));
  assert_eq!(runners.status().state_for(first.path()), RunnerState::Online);
  // One runner per project: the first project's lock is still held by its runner.
  assert!(orchester_runner_lock_held(first.path()));

  assert_eq!(
    runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await,
    ShutdownOutcome::Stopped
  );
  assert!(!orchester_runner_lock_held(first.path()));
  assert!(!orchester_runner_lock_held(second.path()));
}

#[tokio::test(flavor = "multi_thread")]
async fn runner_process_of_a_project_another_app_runs_fails_until_that_app_closes() {
  let project = project();
  let program = runner_program();
  let this_app = Runners::new(program.clone(), Handle::current(), Activity::new(), Clean);
  let other_app = Runners::new(program, Handle::current(), Activity::new(), Clean);
  let mut other_updates = other_app.subscribe();
  other_app.show(Some(&open(project.path())));
  wait_for(&mut other_updates, project.path(), |state| {
    *state == RunnerState::Online
  })
  .await;

  let mut updates = this_app.subscribe();
  this_app.show(Some(&open(project.path())));
  let failed = wait_for(&mut updates, project.path(), |state| *state != RunnerState::Starting).await;
  let expected = RunnerState::Failed {
    cause: RunnerFailure::AlreadyServed,
    attempts: 0,
  };
  assert_eq!(failed, expected);
  // No retries: nothing changes until the user restarts the runner.
  tokio::time::sleep(reconnect_delay(1) * 2).await;
  assert!(!updates.has_changed().expect("the runners outlive the test"));
  assert_eq!(this_app.status().state(), &expected);

  assert_eq!(
    other_app.shutdown(Some(QUIET_STOP_TIMEOUT)).await,
    ShutdownOutcome::Stopped
  );
  this_app.restart();
  wait_for(&mut updates, project.path(), |state| *state == RunnerState::Online).await;
  assert_eq!(
    this_app.shutdown(Some(QUIET_STOP_TIMEOUT)).await,
    ShutdownOutcome::Stopped
  );
}

/// Whether a runner holds the project's runner lock (`.orchester/runner.lock`).
fn orchester_runner_lock_held(project: &Path) -> bool {
  let lock = fs::OpenOptions::new()
    .read(true)
    .write(true)
    .create(true)
    .truncate(false)
    .open(project.join(".orchester").join("runner.lock"))
    .expect("open the lock file");
  match lock.try_lock() {
    Ok(()) => false,
    Err(fs::TryLockError::WouldBlock) => true,
    Err(fs::TryLockError::Error(error)) => panic!("cannot check the lock: {error}"),
  }
}
