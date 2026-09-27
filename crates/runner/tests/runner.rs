//! End-to-end tests against the real `orchester-runner` binary. The desktop connection is a
//! named pipe, which exists on Windows only.
#![cfg(windows)]

use std::{fs, num::NonZeroU32, path::Path, time::Duration};

use orchester_core::{
  id::{IdempotencyKey, ProjectId, RunId, TaskId},
  name::Name,
  project::{Project, ProjectKey},
  tokens::TokenBudget,
};
use orchester_ipc::{
  AttachSession, Command, Event, LaunchError, Rejection, Reply, Request, RequestId, RunnerClient, SessionEnd,
  ShutdownMode, WorkSummary,
};
use orchester_runner::lock::RunnerLock;
use orchester_store::ProjectDb;
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

fn request(id: u64, command: Command) -> Request {
  Request {
    id: RequestId::new(id),
    command,
  }
}

async fn stop(mode: ShutdownMode) {
  let project = project();
  let mut client = launch(project.path()).await;

  client
    .send(&request(1, Command::Ping { nonce: 7 }))
    .await
    .expect("ping");
  assert_eq!(
    next_event(&mut client).await,
    Some(Event::Reply {
      id: RequestId::new(1),
      reply: Reply::Pong { nonce: 7 }
    })
  );
  client
    .send(&request(2, Command::Shutdown { mode }))
    .await
    .expect("shutdown");
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
    Err(LaunchError::ProjectLocked) => {}
    Err(other) => panic!("expected the second runner to report the held lock, got {other}"),
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

#[tokio::test]
async fn runner_logs_to_the_project_log_directory() {
  let project = project();
  let client = launch(project.path()).await;
  let (commands, events, mut process) = client.into_parts();
  drop((commands, events));
  timeout(PATIENCE, process.wait())
    .await
    .expect("runner exits in time")
    .expect("wait");

  let logs: Vec<_> = fs::read_dir(project.path().join(".orchester/logs"))
    .expect("log directory")
    .map(|entry| entry.expect("log entry").path())
    .collect();
  assert_eq!(logs.len(), 1, "{logs:?}");
  let name = logs[0].file_name().and_then(|name| name.to_str()).expect("utf-8 name");
  assert!(name.starts_with("runner-") && name.ends_with(".log"), "{name}");
  assert!(!fs::read_to_string(&logs[0]).expect("read log").is_empty());
}

#[tokio::test]
async fn runner_answers_attach_and_start_run_requests() {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    Name::new("Kontur").expect("name"),
  );
  drop(ProjectDb::create(repo.path(), project).expect("create project"));
  let mut client = launch(repo.path()).await;

  let attach = Command::Attach {
    session: AttachSession::Fresh {
      previous_end: SessionEnd::Clean,
    },
    agent_slots: NonZeroU32::new(2).expect("above zero"),
  };
  client.send(&request(1, attach)).await.expect("attach");
  assert_eq!(
    next_event(&mut client).await,
    Some(Event::Reply {
      id: RequestId::new(1),
      reply: Reply::Attached {
        active_runs: Vec::new(),
        interrupted: Vec::new(),
        continued: Vec::new(),
      }
    })
  );
  assert_eq!(
    next_event(&mut client).await,
    Some(Event::WorkSummary(WorkSummary {
      agent_slots: NonZeroU32::new(2).expect("above zero"),
      runs: Vec::new(),
    }))
  );
  let start = Command::StartRun {
    key: IdempotencyKey::generate(),
    task: TaskId::generate(),
  };
  client.send(&request(2, start)).await.expect("start run");
  assert_eq!(
    next_event(&mut client).await,
    Some(Event::Reply {
      id: RequestId::new(2),
      reply: Reply::Rejected(Rejection::TaskNotFound)
    })
  );
  let run = RunId::generate();
  let controls = [
    Command::PauseRun { run },
    Command::ResumeRun { run },
    Command::CancelRun { run },
    Command::RaiseLimit {
      run,
      budget: TokenBudget::new(2_000_000).expect("above zero"),
      max_duration_ms: 60_000,
    },
  ];
  for (id, command) in (3..).zip(controls) {
    client.send(&request(id, command)).await.expect("run control");
    assert_eq!(
      next_event(&mut client).await,
      Some(Event::Reply {
        id: RequestId::new(id),
        reply: Reply::Rejected(Rejection::RunNotFound)
      })
    );
  }
}
