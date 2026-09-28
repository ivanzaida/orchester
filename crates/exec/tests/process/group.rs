//! Process groups on macOS: every program leads its own, and ending the group ends what the
//! program started too, with SIGKILL for what ignores SIGTERM.

use std::{
  fs,
  path::Path,
  process::Child,
  thread,
  time::{Duration, Instant},
};

use orchester_core::command::{Command, CommandError, CommandRunner};
use orchester_exec::{
  ProcessRunner,
  process_group::{GroupEnd, ProcessGroup},
};
use rustix::process::{Pid, getpgid, test_kill_process};

const PATIENCE: Duration = Duration::from_secs(10);

fn shell(script: &str) -> Command {
  Command::new(Path::new("/bin/sh"), ["-c", script])
}

fn start(script: &str) -> (Child, ProcessGroup) {
  let child = orchester_exec::prepare(&shell(script))
    .expect("prepare")
    .spawn()
    .expect("start sh");
  let group = ProcessGroup::led_by(child.id()).expect("a child's id");
  (child, group)
}

fn pid(id: u32) -> Pid {
  Pid::from_raw(i32::try_from(id).expect("a process id")).expect("not zero")
}

/// Waits until `done` holds, at most [`PATIENCE`].
fn eventually(what: &str, done: impl Fn() -> bool) {
  let deadline = Instant::now() + PATIENCE;
  while !done() {
    assert!(Instant::now() < deadline, "{what} did not happen in time");
    thread::sleep(Duration::from_millis(20));
  }
}

#[test]
fn group_is_led_by_each_program_started() {
  let (mut child, group) = start("sleep 30");
  assert_eq!(getpgid(Some(pid(child.id()))).expect("process group"), pid(child.id()));
  assert_ne!(
    getpgid(None).expect("own group"),
    pid(child.id()),
    "the test process keeps its own group"
  );

  group.kill().expect("kill");
  child.wait().expect("reap");
}

#[test]
fn group_end_ends_what_the_program_left_running() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let pid_file = folder.path().join("background.pid");
  let (mut child, group) = start(&format!("sleep 30 & echo $! > '{}'", pid_file.display()));
  child.wait().expect("the shell exits at once");
  eventually("the background pid is written", || {
    fs::read_to_string(&pid_file).is_ok_and(|text| text.ends_with('\n'))
  });
  let background: u32 = fs::read_to_string(&pid_file)
    .expect("pid file")
    .trim()
    .parse()
    .expect("a pid");
  assert!(test_kill_process(pid(background)).is_ok(), "the background sleep runs");

  assert_eq!(group.end(PATIENCE).expect("end the group"), GroupEnd::Exited);
  assert!(group.is_gone().expect("group state"));
}

#[test]
fn group_members_ignoring_sigterm_are_killed_after_the_grace_period() {
  let (mut child, group) = start("trap '' TERM; sleep 30");
  // Let the shell set its trap before it is signalled.
  thread::sleep(Duration::from_millis(300));

  let started = Instant::now();
  assert_eq!(
    group.end(Duration::from_millis(300)).expect("end the group"),
    GroupEnd::Killed
  );
  assert!(started.elapsed() >= Duration::from_millis(300));
  child.wait().expect("reap");
  eventually("the group is gone", || group.is_gone().expect("group state"));
}

#[test]
fn process_that_runs_too_long_is_stopped_with_what_it_started() {
  let folder = tempfile::tempdir().expect("temporary directory");
  let pid_file = folder.path().join("background.pid");
  let script = format!("sleep 30 & echo $! > '{}'; sleep 30", pid_file.display());
  let runner = ProcessRunner::new(Duration::from_millis(500));

  assert_eq!(runner.run(&shell(&script)), Err(CommandError::TimedOut));

  let background: u32 = fs::read_to_string(&pid_file)
    .expect("pid file")
    .trim()
    .parse()
    .expect("a pid");
  eventually("the background sleep ends", || {
    test_kill_process(pid(background)).is_err()
  });
}
