use std::fs;

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
