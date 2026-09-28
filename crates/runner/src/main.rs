//! `orchester-runner <project-root>`: started by the desktop, one per project.

use std::{io, path::PathBuf, process::ExitCode, sync::Arc};

use orchester_ipc::{EXIT_PROJECT_LOCKED, IpcError};
use orchester_runner::{
  lock::{LockError, RunnerLock},
  log_file, server,
  shutdown_signal::ShutdownSignal,
};
use thiserror::Error;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Agents run on the runtime's workers while database and git calls block one of them briefly.
const WORKER_THREADS: usize = 2;

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
  let project_root = project_root_argument();
  let (log_file, log_error) = match project_root.as_ref().map(|root| log_file::open(root)) {
    Ok(Ok((_, file))) => (Some(file), None),
    Ok(Err(error)) => (None, Some(error)),
    Err(_) => (None, None),
  };
  // The desktop reads stderr: it logs each line and reports it when the runner fails to start.
  // The same events also go to the project's runner log, when the project has one.
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
    .init();
  if let Some(error) = log_error {
    tracing::warn!("runner log file unavailable, logging to stderr only: {error}");
  }
  match run(project_root) {
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

fn run(project_root: Result<PathBuf, RunnerError>) -> Result<(), RunnerError> {
  let project_root = project_root?;
  let _lock = RunnerLock::acquire(&project_root)?;
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(WORKER_THREADS)
    .enable_all()
    .build()
    .map_err(RunnerError::Runtime)?;
  runtime.block_on(async {
    let shutdown = ShutdownSignal::install().map_err(RunnerError::Signal)?;
    Ok(server::serve(&project_root, shutdown).await?)
  })
}

fn project_root_argument() -> Result<PathBuf, RunnerError> {
  let mut arguments = std::env::args_os().skip(1);
  match (arguments.next(), arguments.next()) {
    (Some(root), None) => Ok(PathBuf::from(root)),
    _ => Err(RunnerError::Usage),
  }
}
