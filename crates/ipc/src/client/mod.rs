//! The desktop's side: start a project's runner, connect to it and exchange messages.

use std::{
  io,
  path::{Path, PathBuf},
  process::{ExitStatus, Stdio},
  time::Duration,
};

use thiserror::Error;
use tokio::{
  io::{AsyncBufReadExt, BufReader, ReadHalf, WriteHalf},
  process::{Child, ChildStderr},
  task::JoinHandle,
  time::Instant,
};

use crate::{
  ClientStream, EXIT_PROJECT_LOCKED, Endpoint, Event, IpcError, KillOnCloseJob, Request,
  frame::{FrameReader, FrameWriter},
  handshake, pipe_owner,
};

/// How long a freshly started runner may take to open its pipe.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_RETRY: Duration = Duration::from_millis(20);
/// The desktop is a GUI process; without this flag every console child opens a window.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// All instances of the pipe are busy: another desktop is connected to this runner.
const ERROR_PIPE_BUSY: i32 = 231;
/// Runner diagnostics kept for the error when it exits before accepting the connection.
const STDERR_KEPT_BYTES: usize = 8 * 1024;

#[derive(Debug, Error)]
pub enum LaunchError {
  #[error("cannot start runner {exe}: {source}")]
  Spawn { exe: PathBuf, source: io::Error },
  #[error("cannot place the runner in a job object: {0}")]
  Job(io::Error),
  #[error("the runner exited before accepting the connection ({status}): {stderr}")]
  RunnerExited { status: ExitStatus, stderr: String },
  /// The runner exited with [`EXIT_PROJECT_LOCKED`]: another Orchester app runs this project.
  #[error("another Orchester app is already running this project's runner (the project's runner lock is held)")]
  ProjectLocked,
  #[error("the runner did not open its pipe within {0:?}")]
  ConnectTimeout(Duration),
  #[error("cannot connect to the runner: {0}")]
  Connect(io::Error),
  /// The pipe is served by a process other than the runner that was started.
  #[error("the runner pipe is served by process {found}, not by the runner that was started")]
  ForeignPipeServer { found: u32 },
  #[error(transparent)]
  Ipc(#[from] IpcError),
}

/// A connected runner for one project.
pub struct RunnerClient {
  commands: CommandSender,
  events: EventReceiver,
  process: RunnerProcess,
}

impl RunnerClient {
  /// Starts `runner_exe` for the project at `project_root` inside a kill-on-close job,
  /// connects to it and performs the handshake.
  pub async fn launch(runner_exe: &Path, project_root: &Path) -> Result<Self, LaunchError> {
    let endpoint = Endpoint::for_project(project_root)?;
    let process = RunnerProcess::spawn(runner_exe, project_root)?;
    let (stream, process) = connect(&endpoint, process).await?;
    let (read, write) = tokio::io::split(stream);
    let mut events = FrameReader::new(read);
    let mut commands = FrameWriter::new(write);
    handshake(&mut events, &mut commands).await?;
    Ok(Self {
      commands: CommandSender { writer: commands },
      events: EventReceiver { reader: events },
      process,
    })
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

/// The runner process. Dropping it closes its job, which terminates the runner and every
/// agent it started ("quit now"); the same happens when the desktop process dies.
pub struct RunnerProcess {
  child: Child,
  stderr: JoinHandle<String>,
  _job: KillOnCloseJob,
}

impl RunnerProcess {
  fn spawn(runner_exe: &Path, project_root: &Path) -> Result<Self, LaunchError> {
    let mut child = tokio::process::Command::new(runner_exe)
      .arg(project_root)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::piped())
      .creation_flags(CREATE_NO_WINDOW)
      // Covers the window between spawning and joining the job.
      .kill_on_drop(true)
      .spawn()
      .map_err(|source| LaunchError::Spawn {
        exe: runner_exe.to_path_buf(),
        source,
      })?;
    let job = KillOnCloseJob::new().map_err(LaunchError::Job)?;
    let handle = child
      .raw_handle()
      .ok_or_else(|| LaunchError::Job(io::Error::other("the runner exited before it could join the job")))?;
    job.assign(handle).map_err(LaunchError::Job)?;
    let stderr = child.stderr.take().expect("stderr was configured as piped");
    let stderr = tokio::spawn(drain_stderr(stderr));
    Ok(Self {
      child,
      stderr,
      _job: job,
    })
  }

  /// The operating system process id, while the runner is running.
  pub fn id(&self) -> Option<u32> {
    self.child.id()
  }

  /// Waits for the runner to exit, e.g. after [`Event::Stopped`] and closing the connection.
  pub async fn wait(&mut self) -> io::Result<ExitStatus> {
    self.child.wait().await
  }

  async fn exited(self, status: ExitStatus) -> LaunchError {
    if status.code() == Some(i32::from(EXIT_PROJECT_LOCKED)) {
      return LaunchError::ProjectLocked;
    }
    let stderr = match self.stderr.await {
      Ok(stderr) => stderr,
      Err(error) => format!("runner diagnostics unavailable: {error}"),
    };
    LaunchError::RunnerExited {
      status,
      stderr: stderr.trim().to_owned(),
    }
  }
}

/// Connects once the runner has created its pipe, or reports why it never will.
async fn connect(
  endpoint: &Endpoint,
  mut process: RunnerProcess,
) -> Result<(ClientStream, RunnerProcess), LaunchError> {
  let deadline = Instant::now() + CONNECT_TIMEOUT;
  loop {
    match endpoint.connect() {
      Ok(stream) => {
        let found = pipe_owner::server_process_id(&stream).map_err(LaunchError::Connect)?;
        if process.id() != Some(found) {
          return Err(LaunchError::ForeignPipeServer { found });
        }
        return Ok((stream, process));
      }
      Err(error) if error.kind() == io::ErrorKind::NotFound || error.raw_os_error() == Some(ERROR_PIPE_BUSY) => {}
      Err(error) => return Err(LaunchError::Connect(error)),
    }
    if let Some(status) = process.child.try_wait().map_err(LaunchError::Connect)? {
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
