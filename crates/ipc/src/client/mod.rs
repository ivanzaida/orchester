//! The desktop's side: start a project's runner, connect to it and exchange messages.

mod process;

use std::{
  io,
  path::{Path, PathBuf},
  process::ExitStatus,
  time::Duration,
};

use thiserror::Error;
use tokio::{
  io::{AsyncBufReadExt, BufReader, ReadHalf, WriteHalf},
  process::ChildStderr,
  time::Instant,
};

pub use self::process::RunnerProcess;
use crate::{
  ClientStream, Endpoint, ErrorReports, Event, IpcError, Request,
  frame::{FrameReader, FrameWriter},
  handshake, server_process_id,
};

/// How long a freshly started runner may take to open its endpoint. Generous: under load (parallel
/// builds and tests) a runner can take well over ten seconds to get there.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_RETRY: Duration = Duration::from_millis(20);
/// Runner diagnostics kept for the error when it exits before accepting the connection.
const STDERR_KEPT_BYTES: usize = 8 * 1024;

#[derive(Debug, Error)]
pub enum LaunchError {
  #[error("cannot start runner {exe}: {source}")]
  Spawn { exe: PathBuf, source: io::Error },
  #[cfg(windows)]
  #[error("cannot place the runner in a job object: {0}")]
  Job(io::Error),
  #[error("the runner exited before accepting the connection ({status}): {stderr}")]
  RunnerExited { status: ExitStatus, stderr: String },
  /// The runner exited with [`EXIT_PROJECT_LOCKED`]: another Orchester app runs this project.
  #[error("another Orchester app is already running this project's runner (the project's runner lock is held)")]
  ProjectLocked,
  #[error("the runner did not open its endpoint within {0:?}")]
  ConnectTimeout(Duration),
  #[error("cannot connect to the runner: {0}")]
  Connect(io::Error),
  /// The endpoint is served by a process other than the runner that was started.
  #[error("the runner endpoint is served by process {found}, not by the runner that was started")]
  ForeignPipeServer { found: u32 },
  #[error(transparent)]
  Ipc(#[from] IpcError),
}

impl LaunchError {
  /// Whether starting the runner once more may succeed: not while its program cannot be started
  /// or another runner holds the project, and not when the protocol versions differ.
  pub fn is_transient(&self) -> bool {
    match self {
      Self::RunnerExited { .. } | Self::ConnectTimeout(_) | Self::Connect(_) => true,
      #[cfg(windows)]
      Self::Job(_) => true,
      Self::Ipc(error) => matches!(error, IpcError::Frame(_) | IpcError::ClosedDuringHandshake),
      Self::Spawn { .. } | Self::ProjectLocked | Self::ForeignPipeServer { .. } => false,
    }
  }
}

/// A connected runner for one project.
pub struct RunnerClient {
  commands: CommandSender,
  events: EventReceiver,
  process: RunnerProcess,
}

impl RunnerClient {
  /// Starts `runner_exe` for the project at `project_root` (see [`RunnerProcess`] for how it is
  /// ended), connects to it and performs the handshake. The runner sends error reports only with
  /// `reports` ([`ErrorReports`]). A start that fails for a [transient](LaunchError::is_transient)
  /// reason is tried once more, after the first runner has ended, so two runners never hold the project.
  pub async fn launch(
    runner_exe: &Path,
    project_root: &Path,
    reports: Option<&ErrorReports>,
  ) -> Result<Self, LaunchError> {
    let endpoint = Endpoint::for_project(project_root)?;
    match Self::attempt(runner_exe, project_root, reports, &endpoint).await {
      Err(Failed { error, ended: true }) if error.is_transient() => {
        tracing::warn!(project = %project_root.display(), %error, "the runner did not start; starting it once more");
        Self::attempt(runner_exe, project_root, reports, &endpoint)
          .await
          .map_err(|failed| failed.error)
      }
      result => result.map_err(|failed| failed.error),
    }
  }

  /// One start of the runner; a runner that fails to start is ended before this returns.
  async fn attempt(
    runner_exe: &Path,
    project_root: &Path,
    reports: Option<&ErrorReports>,
    endpoint: &Endpoint,
  ) -> Result<Self, Failed> {
    let mut process =
      RunnerProcess::spawn(runner_exe, project_root, reports).map_err(|error| Failed { error, ended: true })?;
    let error = match open(endpoint, &mut process).await {
      Ok((commands, events)) => {
        return Ok(Self {
          commands,
          events,
          process,
        });
      }
      Err(error) => error,
    };
    let ended = match process.end().await {
      Ok(()) => true,
      Err(end_error) => {
        tracing::warn!(project = %project_root.display(), %end_error, "cannot end the runner that failed to start");
        false
      }
    };
    Err(Failed { error, ended })
  }

  pub async fn send(&mut self, request: &Request) -> Result<(), IpcError> {
    self.commands.send(request).await
  }

  /// The next event, or `None` once the runner closed the connection.
  pub async fn next_event(&mut self) -> Result<Option<Event>, IpcError> {
    self.events.next().await
  }

  /// Separates the halves so commands and events can be handled by different tasks. Dropping
  /// both connection halves closes the connection; the runner then exits.
  pub fn into_parts(self) -> (CommandSender, EventReceiver, RunnerProcess) {
    (self.commands, self.events, self.process)
  }
}

pub struct CommandSender {
  writer: FrameWriter<WriteHalf<ClientStream>>,
}

impl CommandSender {
  pub async fn send(&mut self, request: &Request) -> Result<(), IpcError> {
    Ok(self.writer.write(request).await?)
  }
}

pub struct EventReceiver {
  reader: FrameReader<ReadHalf<ClientStream>>,
}

impl EventReceiver {
  /// The next event, or `None` once the runner closed the connection.
  pub async fn next(&mut self) -> Result<Option<Event>, IpcError> {
    Ok(self.reader.read().await?)
  }
}

/// A failed start, and whether its runner is known to have ended.
struct Failed {
  error: LaunchError,
  ended: bool,
}

/// Connects to the started runner and performs the handshake.
async fn open(endpoint: &Endpoint, process: &mut RunnerProcess) -> Result<(CommandSender, EventReceiver), LaunchError> {
  let stream = connect(endpoint, process).await?;
  let (read, write) = tokio::io::split(stream);
  let mut reader = FrameReader::new(read);
  let mut writer = FrameWriter::new(write);
  handshake(&mut reader, &mut writer).await?;
  Ok((CommandSender { writer }, EventReceiver { reader }))
}

/// Connects once the runner has opened its endpoint, or reports why it never will.
async fn connect(endpoint: &Endpoint, process: &mut RunnerProcess) -> Result<ClientStream, LaunchError> {
  let deadline = Instant::now() + CONNECT_TIMEOUT;
  loop {
    match endpoint.connect().await {
      Ok(stream) => {
        let found = server_process_id(&stream).map_err(LaunchError::Connect)?;
        if process.id() != Some(found) {
          return Err(LaunchError::ForeignPipeServer { found });
        }
        return Ok(stream);
      }
      Err(error) if Endpoint::not_ready(&error) => {}
      Err(error) => return Err(LaunchError::Connect(error)),
    }
    if let Some(status) = process.try_wait().map_err(LaunchError::Connect)? {
      return Err(process.exited(status).await);
    }
    if Instant::now() >= deadline {
      return Err(LaunchError::ConnectTimeout(CONNECT_TIMEOUT));
    }
    tokio::time::sleep(CONNECT_RETRY).await;
  }
}

/// Forwards the runner's log lines to the desktop log and keeps the first few kilobytes for
/// [`LaunchError::RunnerExited`]. Draining also keeps the runner from blocking on a full pipe.
async fn drain_stderr(stderr: ChildStderr) -> String {
  let mut reader = BufReader::new(stderr);
  let mut kept = String::new();
  let mut line = Vec::new();
  loop {
    line.clear();
    match reader.read_until(b'\n', &mut line).await {
      Ok(0) => return kept,
      Ok(_) => {}
      Err(error) => {
        tracing::warn!("cannot read runner diagnostics: {error}");
        return kept;
      }
    }
    let text = String::from_utf8_lossy(&line);
    let text = text.trim_end();
    tracing::info!(target: "orchester_runner", "{text}");
    if kept.len() < STDERR_KEPT_BYTES {
      kept.push_str(text);
      kept.push('\n');
    }
  }
}
