#![cfg(windows)]
use std::{
  fs,
  path::PathBuf,
  process::{Command as Process, Stdio},
  sync::Arc,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  command::Command,
  integration::local::{LocalError, ProcessIdentity, ProcessJournal},
};
use orchester_exec::local_git::{LocalGit, LocalGitError};
use parking_lot::Mutex;

#[derive(Default)]
struct Journal {
  identity: Mutex<Option<ProcessIdentity>>,
  finished: Mutex<bool>,
}
impl ProcessJournal for Journal {
  fn begin(&self) -> Result<u64, LocalError> {
    Ok(1)
  }
  fn started(&self, _: u64, identity: Option<&ProcessIdentity>) -> Result<(), LocalError> {
    *self.identity.lock() = identity.cloned();
    Ok(())
  }
  fn finished(&self, _: u64) -> Result<(), LocalError> {
    *self.finished.lock() = true;
    Ok(())
  }
}
fn run_probe(mode: &str) {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-24-tests");
  fs::create_dir_all(&scratch).expect("scratch");
  let dir = tempfile::tempdir_in(scratch).expect("private probe");
  assert!(
    Process::new("git")
      .args(["init", "-b", "main"])
      .current_dir(dir.path())
      .output()
      .expect("git")
      .status
      .success()
  );
  fs::create_dir(dir.path().join("scratch")).expect("scratch");
  let mut git = LocalGit::new(dir.path(), &dir.path().join("scratch")).expect("local");
  let journal = Arc::new(Journal::default());
  git.bind_journal(journal.clone());
  let probe = env!("CARGO_BIN_EXE_orchester-local-process-probe");
  let foreign_counter = dir.path().join("separate-owned-control");
  let mut control = Process::new(probe)
    .arg("writer")
    .arg(&foreign_counter)
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("separate fixture child");
  let counter = dir.path().join("owned-counter");
  let command = Command::new(std::path::Path::new(probe), [mode, counter.to_str().expect("path")]);
  let start = Instant::now();
  let result = git.check_command(&command, Duration::from_millis(750));
  let elapsed = start.elapsed();
  let length = fs::metadata(&counter).expect("actual child wrote").len();
  thread::sleep(Duration::from_millis(150));
  let final_length = fs::metadata(&counter).expect("counter").len();
  let control_running = control.try_wait().expect("control wait").is_none();
  control
    .kill()
    .expect("stop only the independently owned fixture child by its retained handle");
  control.wait().expect("reap own control");
  if mode == "waiting-parent" {
    assert!(matches!(result, Err(LocalGitError::Process)));
  } else {
    assert!(matches!(
      result,
      Err(LocalGitError::State(LocalError::ProcessUncertain))
    ));
  }
  assert!(
    elapsed < Duration::from_secs(4),
    "pipes and descendants cannot freeze the executor"
  );
  assert_eq!(length, final_length, "owned writer has actually stopped");
  assert!(
    control_running,
    "the executor never terminated the separate control process"
  );
  assert!(
    *journal.finished.lock(),
    "completion records the empty whole job after pipes complete"
  );
  let identity = journal.identity.lock().clone().expect("exact root birth recorded");
  assert!(identity.started > 0);
  assert!(identity.containment.is_some());
  assert!(identity.executor.is_some());
  assert!(
    orchester_exec::local_git::family::reconcile_stopped(&identity).is_err(),
    "an unfinished receipt cannot assume blocked pipe workers of this live executor ended"
  );
  assert!(orchester_exec::local_git::family::stopped(&identity).expect("restart observes job completion"));
}
#[test]
fn local_integration_exited_parent_inherited_pipe_descendant_is_stopped_with_exact_owned_job() {
  run_probe("inherited-pipes");
}
#[test]
fn local_integration_exited_parent_closed_pipe_descendant_is_stopped_without_killing_foreign_control() {
  run_probe("closed-pipes");
}
#[test]
fn local_integration_timeout_stops_owned_parent_and_inherited_pipe_writer_before_restart_receipt() {
  run_probe("waiting-parent");
}
