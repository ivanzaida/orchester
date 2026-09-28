use std::{
  io::Read,
  process::{Command, Stdio},
  sync::mpsc,
  thread,
  time::Duration,
};

use orchester_ipc::{Endpoint, KillOnCloseJob};

#[test]
fn endpoint_names_are_stable_per_project_and_distinct_between_projects() {
  let first = tempfile::tempdir().expect("temporary directory");
  let second = tempfile::tempdir().expect("temporary directory");

  let name = Endpoint::for_project(first.path()).expect("endpoint");
  assert!(name.name().starts_with(r"\\.\pipe\orchester-runner-"));
  let respelled = first
    .path()
    .join(".")
    .join("..")
    .join(first.path().file_name().expect("name"));
  assert_eq!(Endpoint::for_project(&respelled).expect("endpoint"), name);
  assert_ne!(Endpoint::for_project(second.path()).expect("endpoint"), name);
}

#[test]
fn endpoint_requires_an_existing_project_root() {
  let dir = tempfile::tempdir().expect("temporary directory");
  assert!(Endpoint::for_project(&dir.path().join("missing")).is_err());
}

/// `cmd` starts `ping`, which inherits stdout. Reading stdout reaches its end only when both
/// have exited, so an early end proves the whole tree died with the job.
#[test]
fn job_kills_the_process_tree_when_dropped() {
  let job = KillOnCloseJob::new().expect("job");
  let mut child = Command::new("cmd")
    .args(["/C", "ping", "-n", "60", "127.0.0.1"])
    .stdout(Stdio::piped())
    .spawn()
    .expect("start cmd");
  job
    .assign(std::os::windows::io::AsRawHandle::as_raw_handle(&child))
    .expect("assign");
  let mut stdout = child.stdout.take().expect("piped stdout");
  let (done, finished) = mpsc::channel();
  thread::spawn(move || {
    let mut output = Vec::new();
    let result = stdout.read_to_end(&mut output);
    done.send(result.map(|_| output)).expect("report");
  });
  // Let ping start so the tree has two processes.
  thread::sleep(Duration::from_millis(500));
  assert!(
    child.try_wait().expect("poll").is_none(),
    "cmd ended before the job closed"
  );

  drop(job);

  finished
    .recv_timeout(Duration::from_secs(10))
    .expect("stdout closed, so cmd and ping are gone")
    .expect("read output");
  assert!(child.wait().expect("wait").code().is_some());
}
