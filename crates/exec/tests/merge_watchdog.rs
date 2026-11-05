//! The watchdog on a merge's git commands (ORC-365, `orchester_exec::WatchedRunner`): a program that writes nothing for
//! the silence it is given is stopped with everything it started — its job on Windows, its process group on macOS —
//! and ends as silent; one that answers runs to its end. `cargo test -p orchester-exec --features test-program --test
//! merge_watchdog`.

use std::{
  fs,
  path::{Path, PathBuf},
  process::{Command as Process, Stdio},
  thread,
  time::{Duration, Instant},
};

use orchester_core::command::{Command, CommandError, CommandRunner};
use orchester_exec::WatchedRunner;

const SILENCE: Duration = Duration::from_millis(500);

fn scratch() -> tempfile::TempDir {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orc-365-watchdog-tests");
  fs::create_dir_all(&scratch).expect("scratch");
  tempfile::tempdir_in(scratch).expect("private folder")
}

#[test]
fn merge_watchdog_stops_a_silent_program_with_everything_it_started() {
  let dir = scratch();
  let probe = env!("CARGO_BIN_EXE_orchester-local-process-probe");
  // A program of the test's own, outside the watched one, that must keep running.
  let mut control = Process::new(probe)
    .arg("writer")
    .arg(dir.path().join("control"))
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
    .expect("control program");
  // The probe waits for a child of its own that writes to a file and never to the pipes: as git that hangs on a
  // disk, it writes nothing.
  let counter = dir.path().join("counter");
  let command = Command::new(
    Path::new(probe),
    ["waiting-parent", counter.to_str().expect("a Unicode path")],
  );
  let started = Instant::now();
  let ended = WatchedRunner::new(Duration::from_secs(60), SILENCE).run(&command);
  let waited = started.elapsed();
  assert_eq!(ended, Err(CommandError::Silent { after: SILENCE }));
  assert!(waited >= SILENCE, "stopped only once silent for the whole silence");
  assert!(waited < Duration::from_secs(5), "stopped soon after: {waited:?}");
  let written = fs::metadata(&counter).expect("the child wrote").len();
  thread::sleep(Duration::from_millis(200));
  assert_eq!(
    fs::metadata(&counter).expect("counter").len(),
    written,
    "the child the program started was stopped with it"
  );
  let control_running = control.try_wait().expect("control state").is_none();
  control.kill().expect("stop the control program by its handle");
  control.wait().expect("reap the control program");
  assert!(
    control_running,
    "nothing outside the program's job or group was stopped"
  );
}

#[test]
fn merge_watchdog_lets_a_program_that_answers_finish() {
  let print_args = env!("CARGO_BIN_EXE_orchester-print-args");
  let command = Command::new(Path::new(print_args), ["merge", "--ff-only"]);
  let output = WatchedRunner::new(Duration::from_secs(60), SILENCE)
    .run(&command)
    .expect("ran to its end");
  assert!(output.success());
  assert_eq!(output.stdout, "\"merge\"\n\"--ff-only\"\n");
}
