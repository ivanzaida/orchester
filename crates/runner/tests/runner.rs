//! End-to-end tests against the real `orchester-runner` binary.

use std::{fs, path::Path, time::Duration};

use orchester_ipc::{Command, Event, LaunchError, RunnerClient, ShutdownMode};
use orchester_runner::lock::RunnerLock;
use tempfile::TempDir;
use tokio::time::timeout;

const RUNNER: &str = env!("CARGO_BIN_EXE_orchester-runner");
const PATIENCE: Duration = Duration::from_secs(10);

fn project() -> TempDir {
  let project = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(project.path().join(".orchester")).expect("create .orchester");
  project
}

async fn launch(project: &Path) -> RunnerClient {
  timeout(PATIENCE, RunnerClient::launch(Path::new(RUNNER), project))
    .await
    .expect("launch in time")
    .expect("launch")
}

async fn next_event(client: &mut RunnerClient) -> Option<Event> {
  timeout(PATIENCE, client.next_event())
    .await
    .expect("event in time")
    .expect("read event")
}

async fn stop(mode: ShutdownMode) {
  let project = project();
  let mut client = launch(project.path()).await;

  client.send(&Command::Ping { nonce: 7 }).await.expect("ping");
  assert_eq!(next_event(&mut client).await, Some(Event::Pong { nonce: 7 }));
  client.send(&Command::Shutdown { mode }).await.expect("shutdown");
  assert_eq!(next_event(&mut client).await, Some(Event::Stopped { mode }));

  let (commands, events, mut process) = client.into_parts();
  drop((commands, events));
  let status = timeout(PATIENCE, process.wait())
    .await
    .expect("runner exits in time")
    .expect("wait");
  assert!(status.success(), "runner exit status {status}");
}

#[tokio::test]
async fn runner_answers_ping_and_stops_at_checkpoints() {
  stop(ShutdownMode::FinishAtCheckpoints).await;
}

#[tokio::test]
async fn runner_answers_ping_and_stops_immediately() {
  stop(ShutdownMode::Immediate).await;
}

#[tokio::test]
async fn runner_exits_when_the_desktop_disconnects() {
  let project = project();
  let client = launch(project.path()).await;

  let (commands, events, mut process) = client.into_parts();
  drop((commands, events));
  let status = timeout(PATIENCE, process.wait())
    .await
    .expect("runner exits in time")
    .expect("wait");
  assert!(status.success(), "runner exit status {status}");
}

#[tokio::test]
async fn second_runner_for_a_project_fails_clearly() {
  let project = project();
  let _first = launch(project.path()).await;

  match RunnerClient::launch(Path::new(RUNNER), project.path()).await {
    Err(LaunchError::RunnerExited { status, stderr }) => {
      assert!(!status.success());
      assert!(stderr.contains("another runner is already running"), "stderr: {stderr}");
    }
    Err(other) => panic!("expected the second runner to exit, got {other}"),
    Ok(_) => panic!("a second runner connected"),
  }
}

#[tokio::test]
async fn runner_refuses_an_uninitialized_project() {
  let project = tempfile::tempdir().expect("temporary directory");
  match RunnerClient::launch(Path::new(RUNNER), project.path()).await {
    Err(LaunchError::RunnerExited { stderr, .. }) => {
      assert!(stderr.contains("is not an Orchester project"), "stderr: {stderr}");
    }
    Err(other) => panic!("expected the runner to exit, got {other}"),
    Ok(_) => panic!("runner accepted an uninitialized project"),
  }
}

/// Dropping the process handle closes the desktop's job, as a desktop crash would. The runner
/// must die even though the connection is still open.
#[tokio::test]
async fn closing_the_desktop_job_kills_the_runner() {
  let project = project();
  let client = launch(project.path()).await;

  let (_commands, mut events, process) = client.into_parts();
  drop(process);

  let end = timeout(PATIENCE, events.next()).await.expect("connection ends in time");
  assert!(matches!(end, Ok(None) | Err(_)), "unexpected event {end:?}");
  // The runner's lock is released only when its process is gone.
  timeout(PATIENCE, async {
    while RunnerLock::acquire(project.path()).is_err() {
      tokio::time::sleep(Duration::from_millis(50)).await;
    }
  })
  .await
  .expect("runner lock released");
}

#[tokio::test]
async fn the_desktop_refuses_a_pipe_served_by_another_process() {
  let project = project();
  let endpoint = orchester_ipc::Endpoint::for_project(project.path()).expect("endpoint");
  let squatter = tokio::net::windows::named_pipe::ServerOptions::new()
    .first_pipe_instance(true)
    .create(endpoint.name())
    .expect("squat on the pipe name");
  let accepting = tokio::spawn(async move {
    let _connected = squatter.connect().await;
    squatter
  });

  let result = timeout(PATIENCE, RunnerClient::launch(Path::new(RUNNER), project.path()))
    .await
    .expect("launch returns in time");
  assert!(
    matches!(result, Err(LaunchError::ForeignPipeServer { found }) if found == std::process::id()),
    "{:?}",
    result.err()
  );
  drop(accepting);
}
