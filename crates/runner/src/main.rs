//! `orchester-runner <project-root>`: started by the desktop, one per project.

use std::{io, path::PathBuf, process::ExitCode, sync::Arc};

use orchester_ipc::IpcError;
use orchester_runner::{
  lock::{LockError, RunnerLock},
  log_file,
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
  #[error(transparent)]
  Ipc(#[from] IpcError),
  /// The desktop connection is a named pipe, which exists on Windows only (`AGENTS.md`, known gaps).
  #[cfg(not(windows))]
  #[error("the runner cannot talk to the desktop on this platform yet")]
  TransportUnsupported,
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
      ExitCode::FAILURE
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
  serve(&runtime, &project_root)
}

#[cfg(windows)]
fn serve(runtime: &tokio::runtime::Runtime, project_root: &std::path::Path) -> Result<(), RunnerError> {
  Ok(runtime.block_on(orchester_runner::server::serve(project_root))?)
}

#[cfg(not(windows))]
fn serve(_runtime: &tokio::runtime::Runtime, _project_root: &std::path::Path) -> Result<(), RunnerError> {
  Err(RunnerError::TransportUnsupported)
}

fn project_root_argument() -> Result<PathBuf, RunnerError> {
  let mut arguments = std::env::args_os().skip(1);
  match (arguments.next(), arguments.next()) {
    (Some(root), None) => Ok(PathBuf::from(root)),
    _ => Err(RunnerError::Usage),
  }
}
