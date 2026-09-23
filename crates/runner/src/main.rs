//! `orchester-runner <project-root>`: started by the desktop, one per project.

use std::{io, path::PathBuf, process::ExitCode};

use orchester_ipc::IpcError;
use orchester_runner::{
  lock::{LockError, RunnerLock},
  server,
};
use thiserror::Error;
use tracing_subscriber::EnvFilter;

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
}

fn main() -> ExitCode {
  // The desktop reads stderr: it logs each line and reports it when the runner fails to start.
  tracing_subscriber::fmt()
    .with_writer(io::stderr)
    .with_ansi(false)
    .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
    .init();
  match run() {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      tracing::error!("{error}");
      ExitCode::FAILURE
    }
  }
}

fn run() -> Result<(), RunnerError> {
  let project_root = project_root_argument()?;
  let _lock = RunnerLock::acquire(&project_root)?;
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .map_err(RunnerError::Runtime)?;
  Ok(runtime.block_on(server::serve(&project_root))?)
}

fn project_root_argument() -> Result<PathBuf, RunnerError> {
  let mut arguments = std::env::args_os().skip(1);
  match (arguments.next(), arguments.next()) {
    (Some(root), None) => Ok(PathBuf::from(root)),
    _ => Err(RunnerError::Usage),
  }
}
