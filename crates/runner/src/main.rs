//! `orchester-runner <project-root>`: started by the desktop, one per project. Started by Claude
//! Code as `orchester-runner --claude-permission-prompt`, or by Codex as `orchester-runner
//! --orchester-tools`, it is the relay of a step's permission prompt and Orchester tools instead
//! ([`orchester_runner::approval_relay`]).

use std::{io, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use orchester_ipc::{EXIT_PROJECT_LOCKED, IpcError};
use orchester_runner::{
  WORKER_THREADS, approval_relay, error_reports,
  lock::{LockError, RunnerLock},
  log_file, private_file, server,
  shutdown_signal::ShutdownSignal,
};
use orchester_store::ProjectDb;
use thiserror::Error;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// How long blocking work still running when the runner stops may take to finish.
const BLOCKING_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug, Error)]
enum RunnerError {
  #[error("usage: orchester-runner <project-root>")]
  Usage,
  #[error(transparent)]
  Lock(#[from] LockError),
  #[error("cannot start the async runtime: {0}")]
  Runtime(io::Error),
  #[error("cannot listen for shutdown signals: {0}")]
  Signal(io::Error),
  #[error(transparent)]
  Ipc(#[from] IpcError),
}

fn main() -> ExitCode {
  match std::env::args_os().nth(1) {
    Some(first) if first == approval_relay::RELAY_FLAG => return approval_relay::run(true),
    Some(first) if first == approval_relay::TOOLS_FLAG => return approval_relay::run(false),
    _ => {}
  }
  let project_root = project_root_argument();
  let (log_file, log_error) = match project_root.as_ref().map(|root| log_file::open(root)) {
    Ok(Ok((_, file))) => (Some(file), None),
    Ok(Err(error)) => (None, Some(error)),
    Err(_) => (None, None),
  };
  // The desktop reads stderr: it logs each line and reports it when the runner fails to start.
  // The same events also go to the project's runner log, when the project has one, and each
  // ERROR line is an error report while the reports' client runs.
  let file_layer = log_file.map(|file| {
    tracing_subscriber::fmt::layer()
      .with_ansi(false)
      .with_writer(Arc::new(file))
  });
  tracing_subscriber::registry()
    .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
    .with(
      tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(io::stderr),
    )
    .with(file_layer)
    .with(error_reports::layer())
    .init();
  // Kept to the end of `main`, whose drop sends what is still pending.
  let reports = error_reports::Reports::start();
  if let Some(error) = log_error {
    tracing::warn!("runner log file unavailable, logging to stderr only: {error}");
  }
  match run(project_root, reports.as_ref().map(error_reports::Reports::switch)) {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      tracing::error!("{error}");
      error.exit_code()
    }
  }
}

impl RunnerError {
  /// The desktop tells another app's runner from other failures by the exit code.
  fn exit_code(&self) -> ExitCode {
    match self {
      Self::Lock(LockError::AlreadyRunning(_)) => ExitCode::from(EXIT_PROJECT_LOCKED),
      _ => ExitCode::FAILURE,
    }
  }
}

fn run(project_root: Result<PathBuf, RunnerError>, reports: Option<error_reports::Switch>) -> Result<(), RunnerError> {
  let project_root = project_root?;
  let _lock = RunnerLock::acquire(&project_root)?;
  remove_leftover_files(&project_root);
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(WORKER_THREADS)
    .enable_all()
    .build()
    .map_err(RunnerError::Runtime)?;
  let served = runtime.block_on(async {
    let shutdown = ShutdownSignal::install().map_err(RunnerError::Signal)?;
    Ok(server::serve(&project_root, shutdown, reports).await?)
  });
  // Blocking work still running (a codebase index build, which can take a minute) does not hold
  // up the exit: it is abandoned with the process. A build is published by one rename only once
  // complete, so an abandoned one leaves only its staging folder, which the next build removes.
  runtime.shutdown_timeout(BLOCKING_GRACE);
  served
}

/// Files with secret values an earlier runner of the project left behind when it ended abnormally. The lock
/// is held, so no other runner of the project uses them.
fn remove_leftover_files(project_root: &std::path::Path) {
  let removed = ProjectDb::open(project_root)
    .map_err(|error| error.to_string())
    .and_then(|db| private_file::remove_leftovers(db.project().id()).map_err(|error| error.to_string()));
  if let Err(error) = removed {
    tracing::warn!("cannot remove files an earlier runner left behind: {error}");
  }
}

fn project_root_argument() -> Result<PathBuf, RunnerError> {
  let mut arguments = std::env::args_os().skip(1);
  match (arguments.next(), arguments.next()) {
    (Some(root), None) => Ok(PathBuf::from(root)),
    _ => Err(RunnerError::Usage),
  }
}
