//! Supervised child processes on macOS, where each child leads its own process group. A
//! background `sleep` holds the output pipes; the exit is reported only after both pipes close,
//! so a prompt exit event proves the whole group was ended. The Windows counterpart is
//! `supervisor.rs`.
#![cfg(unix)]

use std::{path::Path, time::Duration};

use orchester_core::{command::Command, connection::stream::OutputStream};
use orchester_runner::supervisor::{ProcessEvent, ProcessExit, Supervised};
use tokio::time::{Instant, timeout};

/// Well below the background `sleep`, well above the process groups' grace period.
const PATIENCE: Duration = Duration::from_secs(15);

fn shell(script: &str) -> Command {
  Command::new(Path::new("/bin/sh"), ["-c", script])
}

async fn next(process: &mut Supervised) -> ProcessEvent {
  timeout(PATIENCE, process.next())
    .await
    .expect("event in time")
    .expect("the exit is reported")
}

/// Waits for the line `started`, so the script has set up what the test ends.
async fn wait_for_start(process: &mut Supervised) {
  loop {
    if let ProcessEvent::Output { stream, line } = next(process).await
      && stream == OutputStream::Stdout
      && line == "started"
    {
      return;
    }
  }
}

async fn exit(process: &mut Supervised) -> ProcessExit {
  loop {
    if let ProcessEvent::Exited(exit) = next(process).await {
      return exit;
    }
  }
}

#[tokio::test]
async fn supervisor_terminate_ends_the_process_group() {
  let mut process = Supervised::start(&shell("sleep 60 & echo started; wait")).expect("start");
  wait_for_start(&mut process).await;

  process.terminate();

  assert_eq!(exit(&mut process).await, ProcessExit::Terminated);
}

#[tokio::test]
async fn supervisor_ends_what_an_exited_program_left_running() {
  let mut process = Supervised::start(&shell("sleep 60 & echo started")).expect("start");
  wait_for_start(&mut process).await;

  assert_eq!(exit(&mut process).await, ProcessExit::Exited { code: Some(0) });
}

#[tokio::test]
async fn supervisor_kills_a_program_that_ignores_sigterm() {
  let mut process = Supervised::start(&shell("trap '' TERM; echo started; sleep 60")).expect("start");
  wait_for_start(&mut process).await;

  let started = Instant::now();
  process.terminate();

  assert_eq!(exit(&mut process).await, ProcessExit::Terminated);
  assert!(
    started.elapsed() >= orchester_exec::process_group::TERMINATE_GRACE,
    "SIGKILL comes only after the grace period"
  );
}
