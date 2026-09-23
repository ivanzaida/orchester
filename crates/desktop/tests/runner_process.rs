//! The desktop's runner handling against the real `orchester-runner` program.

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::Duration,
};

use orchester_desktop::{
  app_paths::runner_beside,
  close::{CloseKind, session_end},
  runner::{
    QUIET_STOP_TIMEOUT, RunnerStatus, Runners, ShutdownOutcome,
    state::{RunnerFailure, RunnerState},
  },
};
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

fn project() -> TempDir {
  let project = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(project.path().join(".orchester")).expect("create .orchester");
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
  let runners = Runners::new(runner_program(), Handle::current());
  let mut updates = runners.subscribe();

  runners.show(Some(project.path()));
  let online = wait_for(&mut updates, project.path(), |state| *state != RunnerState::Starting).await;
  assert_eq!(online, RunnerState::Online { reconnected: false });

  let outcome = runners.shutdown(Some(QUIET_STOP_TIMEOUT)).await;
  assert_eq!(outcome, ShutdownOutcome::Stopped);
  assert_eq!(runners.status().state(), &RunnerState::Stopped);
  assert_eq!(session_end(CloseKind::Quiet, outcome), SessionEnd::Clean);
}

#[tokio::test(flavor = "multi_thread")]
async fn runner_process_missing_program_fails_visibly() {
  let project = project();
  let missing = project.path().join("orchester-runner.exe");
  let runners = Runners::new(missing.clone(), Handle::current());
  let mut updates = runners.subscribe();

  runners.show(Some(project.path()));
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
