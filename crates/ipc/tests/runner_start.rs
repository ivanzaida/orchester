//! Starting a project's runner against `orchester-fake-runner`: a late runner is waited for, a
//! runner that fails to start is started once more, and a held project is not. Each start tells
//! the runner whether it sends error reports.
//!
//! `cargo test -p orchester-ipc --features test-runner --test runner_start runner_start_`

use std::{
  ffi::OsStr,
  fs, io,
  path::Path,
  sync::{Arc, OnceLock},
};

use orchester_ipc::{ErrorReports, LaunchError, RunnerClient};
use parking_lot::Mutex;
use tempfile::TempDir;
use tracing_subscriber::fmt::MakeWriter;

const FAKE_RUNNER: &str = env!("CARGO_BIN_EXE_orchester-fake-runner");
/// What the client logs before it starts the runner once more.
const RETRY_WARNING: &str = "the runner did not start; starting it once more";

/// Everything this test binary logs; tests tell their lines apart by their project's path.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl io::Write for Captured {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.0.lock().extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

impl<'a> MakeWriter<'a> for Captured {
  type Writer = Self;

  fn make_writer(&'a self) -> Self::Writer {
    self.clone()
  }
}

fn captured() -> &'static Captured {
  static CAPTURED: OnceLock<Captured> = OnceLock::new();
  CAPTURED.get_or_init(|| {
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
      .with_ansi(false)
      .with_writer(captured.clone())
      .finish();
    tracing::subscriber::set_global_default(subscriber).expect("the only subscriber of this test binary");
    captured
  })
}

/// A project whose fake runner follows `plan` (see `tests/support/fake_runner.rs`).
fn project(plan: &str) -> TempDir {
  captured();
  let project = tempfile::tempdir().expect("temporary directory");
  fs::write(project.path().join("fake-runner"), plan).expect("write the plan");
  project
}

fn start_lines(project: &Path) -> Vec<String> {
  fs::read_to_string(project.join("starts"))
    .expect("the fake runner notes every start")
    .lines()
    .map(str::to_owned)
    .collect()
}

fn starts(project: &Path) -> usize {
  start_lines(project).len()
}

fn retry_warnings(project: &Path) -> usize {
  let log = String::from_utf8_lossy(&captured().0.lock()).into_owned();
  let project = project.display().to_string();
  log
    .lines()
    .filter(|line| line.contains(RETRY_WARNING) && line.contains(&project))
    .count()
}

async fn launch(project: &Path) -> Result<RunnerClient, LaunchError> {
  RunnerClient::launch(Path::new(FAKE_RUNNER), project, None).await
}

#[tokio::test]
async fn runner_start_tells_the_runner_whether_it_sends_error_reports() {
  let reporting = project("exit-first");
  let reports = ErrorReports::new("qa", "orchester@1.2.3").expect("both are text");

  let client = RunnerClient::launch(Path::new(FAKE_RUNNER), reporting.path(), Some(&reports)).await;

  assert!(client.is_ok(), "{:?}", client.err());
  assert_eq!(
    start_lines(reporting.path()),
    ["start qa orchester@1.2.3"; 2],
    "the second start is told too"
  );

  let silent = project("delay 0");
  let client = launch(silent.path()).await;
  assert!(client.is_ok(), "{:?}", client.err());
  assert_eq!(start_lines(silent.path()), ["start"]);
}

#[test]
fn runner_start_reports_need_an_environment_and_a_release() {
  let qa = Some(OsStr::new("qa"));
  let release = Some(OsStr::new("orchester@1.2.3"));
  assert_eq!(
    ErrorReports::from_variables(qa, release),
    ErrorReports::new("qa", "orchester@1.2.3")
  );
  assert_eq!(ErrorReports::from_variables(None, release), None);
  assert_eq!(ErrorReports::from_variables(qa, None), None);
  assert_eq!(ErrorReports::from_variables(Some(OsStr::new("")), release), None);
  assert_eq!(ErrorReports::new("qa", ""), None);
}

/// Slow by design: the runner answers after eleven seconds.
#[tokio::test]
async fn runner_start_slow_accepts_a_runner_that_answers_after_more_than_ten_seconds() {
  let project = project("delay 11");

  let client = launch(project.path()).await;

  assert!(client.is_ok(), "{:?}", client.err());
  assert_eq!(starts(project.path()), 1);
  assert_eq!(retry_warnings(project.path()), 0);
}

#[tokio::test]
async fn runner_start_retries_once_a_runner_that_exited() {
  let project = project("exit-first");

  let client = launch(project.path()).await;

  assert!(client.is_ok(), "{:?}", client.err());
  assert_eq!(starts(project.path()), 2);
  assert_eq!(retry_warnings(project.path()), 1);
}

#[tokio::test]
async fn runner_start_gives_up_after_the_second_attempt() {
  let project = project("exit");

  let error = launch(project.path()).await.err();

  assert!(matches!(error, Some(LaunchError::RunnerExited { .. })), "{error:?}");
  assert_eq!(starts(project.path()), 2);
  assert_eq!(retry_warnings(project.path()), 1);
}

#[tokio::test]
async fn runner_start_does_not_retry_a_project_another_runner_holds() {
  let project = project("locked");

  let error = launch(project.path()).await.err();

  assert!(matches!(error, Some(LaunchError::ProjectLocked)), "{error:?}");
  assert_eq!(starts(project.path()), 1);
  assert_eq!(retry_warnings(project.path()), 0);
}

#[tokio::test]
async fn runner_start_does_not_retry_a_missing_program() {
  let project = project("exit");

  let error = RunnerClient::launch(&project.path().join("missing-runner"), project.path(), None)
    .await
    .err();

  assert!(matches!(error, Some(LaunchError::Spawn { .. })), "{error:?}");
  assert_eq!(retry_warnings(project.path()), 0);
}
