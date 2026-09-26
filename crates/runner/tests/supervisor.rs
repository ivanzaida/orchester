//! Supervised child processes. Uses `cmd`, so Windows only; the macOS containment is a known gap.
#![cfg(windows)]

use std::{fs, path::Path, time::Duration};

use orchester_core::{command::Command, connection::stream::OutputStream};
use orchester_runner::supervisor::{ProcessEvent, ProcessExit, Supervised, SupervisorError};
use tokio::time::timeout;

const PATIENCE: Duration = Duration::from_secs(10);

/// Events of the process until and including its exit.
async fn collect(process: &mut Supervised) -> Vec<ProcessEvent> {
  let mut collected = Vec::new();
  loop {
    let event = timeout(PATIENCE, process.next())
      .await
      .expect("event in time")
      .expect("the exit is reported");
    let exited = matches!(event, ProcessEvent::Exited(_));
    collected.push(event);
    if exited {
      return collected;
    }
  }
}

fn lines(events: &[ProcessEvent], wanted: OutputStream) -> Vec<&str> {
  events
    .iter()
    .filter_map(|event| match event {
      ProcessEvent::Output { stream, line } if *stream == wanted => Some(line.as_str()),
      _ => None,
    })
    .collect()
}

fn cmd<const N: usize>(dir: &Path, args: [&str; N]) -> Command {
  Command::new(Path::new("cmd"), args).with_current_dir(dir)
}

#[tokio::test]
async fn supervisor_streams_output_lines_and_reports_the_exit_code() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let script = [
    "/C", "echo", "first&", "echo", "second&", "1>&2", "echo", "oops&", "exit", "/B", "3",
  ];
  let mut process = Supervised::start(&cmd(dir.path(), script)).expect("start");
  let events = collect(&mut process).await;

  assert_eq!(lines(&events, OutputStream::Stdout), ["first", "second"]);
  assert_eq!(lines(&events, OutputStream::Stderr), ["oops"]);
  assert_eq!(
    events.last(),
    Some(&ProcessEvent::Exited(ProcessExit::Exited { code: Some(3) }))
  );
  assert_eq!(timeout(PATIENCE, process.next()).await.expect("in time"), None);
}

#[tokio::test]
async fn supervisor_runs_the_child_in_the_working_directory_with_its_input() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut process = Supervised::start(&cmd(dir.path(), ["/C", "cd"])).expect("start");
  let events = collect(&mut process).await;
  let printed = lines(&events, OutputStream::Stdout);
  assert_eq!(printed.len(), 1);
  assert_eq!(
    fs::canonicalize(printed[0]).expect("printed directory exists"),
    fs::canonicalize(dir.path()).expect("canonical directory")
  );

  let echo = cmd(dir.path(), ["/C", "findstr", "x"]).with_stdin("a x b\nno\n".to_owned());
  let mut process = Supervised::start(&echo).expect("start");
  let events = collect(&mut process).await;
  assert_eq!(lines(&events, OutputStream::Stdout), ["a x b"]);
}

/// `cmd` starts `ping`, which holds the output pipes; the exit is reported only after both
/// pipes close, so a prompt `Terminated` proves the whole tree was ended.
#[tokio::test]
async fn supervisor_terminates_the_process_tree() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut process = Supervised::start(&cmd(dir.path(), ["/C", "ping", "-n", "60", "127.0.0.1"])).expect("start");
  tokio::time::sleep(Duration::from_millis(500)).await;
  process.terminate();

  let events = collect(&mut process).await;
  assert_eq!(events.last(), Some(&ProcessEvent::Exited(ProcessExit::Terminated)));
}

#[tokio::test]
async fn supervisor_reports_a_program_that_cannot_start() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let missing = Command::new(Path::new("orchester-no-such-program"), [""; 0]).with_current_dir(dir.path());
  assert!(matches!(
    Supervised::start(&missing),
    Err(SupervisorError::Spawn { .. })
  ));
}
