use std::{
  fs,
  process::{Command, Stdio},
};

use orchester_ipc::EXIT_PROJECT_LOCKED;
use orchester_runner::lock::{LockError, RunnerLock};

#[test]
fn runner_lock_is_exclusive_until_released() {
  let project = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(project.path().join(".orchester")).expect("create .orchester");

  let held = RunnerLock::acquire(project.path()).expect("first lock");
  assert!(matches!(
    RunnerLock::acquire(project.path()),
    Err(LockError::AlreadyRunning(_))
  ));

  drop(held);
  RunnerLock::acquire(project.path()).expect("lock after release");
}

#[test]
fn runner_lock_requires_an_initialized_project() {
  let project = tempfile::tempdir().expect("temporary directory");
  assert!(matches!(
    RunnerLock::acquire(project.path()),
    Err(LockError::NotInitialized(_))
  ));
}

/// The desktop tells "another app runs this project" from other start failures by this code.
#[test]
fn runner_lock_held_by_another_runner_ends_the_runner_with_its_own_exit_code() {
  let project = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(project.path().join(".orchester")).expect("create .orchester");
  let _held = RunnerLock::acquire(project.path()).expect("first lock");

  let status = Command::new(env!("CARGO_BIN_EXE_orchester-runner"))
    .arg(project.path())
    .stderr(Stdio::null())
    .status()
    .expect("run the runner");
  assert_eq!(status.code(), Some(i32::from(EXIT_PROJECT_LOCKED)));
}
