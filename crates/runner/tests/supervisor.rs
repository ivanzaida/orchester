use std::{fs, path::Path, time::Duration};

use orchester_ipc::{OutputStream, ProcessEvent, ProcessExit, ProcessId};
use orchester_runner::supervisor::{Supervisor, SupervisorError};
use tokio::{sync::mpsc, time::timeout};

const PATIENCE: Duration = Duration::from_secs(10);

/// Events of `process` until and including its exit.
async fn collect(events: &mut mpsc::Receiver<ProcessEvent>, process: ProcessId) -> Vec<ProcessEvent> {
  let mut collected = Vec::new();
  loop {
    let event = timeout(PATIENCE, events.recv())
      .await
      .expect("event in time")
      .expect("supervisor alive");
    let exited = matches!(&event, ProcessEvent::Exited { process: id, .. } if *id == process);
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
      ProcessEvent::Output { stream, line, .. } if *stream == wanted => Some(line.as_str()),
      _ => None,
    })
    .collect()
}

#[tokio::test]
async fn supervisor_streams_output_lines_and_reports_the_exit_code() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let (mut supervisor, mut events) = Supervisor::new();

  let script = [
    "/C", "echo", "first&", "echo", "second&", "1>&2", "echo", "oops&", "exit", "/B", "3",
  ];
  let id = supervisor.spawn(Path::new("cmd"), script, dir.path()).expect("spawn");
  let events = collect(&mut events, id).await;

  assert_eq!(lines(&events, OutputStream::Stdout), ["first", "second"]);
  assert_eq!(lines(&events, OutputStream::Stderr), ["oops"]);
  assert_eq!(
    events.last(),
    Some(&ProcessEvent::Exited {
      process: id,
      exit: ProcessExit::Exited { code: Some(3) },
    })
  );
  assert!(matches!(supervisor.terminate(id), Err(SupervisorError::NotRunning(_))));
}

#[tokio::test]
async fn supervisor_runs_the_child_in_the_working_directory() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let (mut supervisor, mut events) = Supervisor::new();

  let id = supervisor
    .spawn(Path::new("cmd"), ["/C", "cd"], dir.path())
    .expect("spawn");
  let events = collect(&mut events, id).await;

  let printed = lines(&events, OutputStream::Stdout);
  assert_eq!(printed.len(), 1);
  assert_eq!(
    fs::canonicalize(printed[0]).expect("printed directory exists"),
    fs::canonicalize(dir.path()).expect("canonical directory")
  );
}

/// `cmd` starts `ping`, which holds the output pipes; the exit is reported only after both
/// pipes close, so a prompt `Terminated` proves the whole tree was killed.
#[tokio::test]
async fn supervisor_terminates_the_process_tree() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let (mut supervisor, mut events) = Supervisor::new();

  let id = supervisor
    .spawn(Path::new("cmd"), ["/C", "ping", "-n", "60", "127.0.0.1"], dir.path())
    .expect("spawn");
  tokio::time::sleep(Duration::from_millis(500)).await;
  supervisor.terminate(id).expect("terminate");

  let events = collect(&mut events, id).await;
  assert_eq!(
    events.last(),
    Some(&ProcessEvent::Exited {
      process: id,
      exit: ProcessExit::Terminated,
    })
  );
}

#[tokio::test]
async fn supervisor_reports_a_program_that_cannot_start() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let (mut supervisor, _events) = Supervisor::new();

  let result = supervisor.spawn(Path::new("orchester-no-such-program"), [""; 0], dir.path());
  assert!(matches!(result, Err(SupervisorError::Spawn { .. })));
}
