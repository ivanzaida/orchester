//! Starts child processes, streams their output as [`ProcessEvent`]s and terminates them.
//!
//! Each child gets its own kill-on-close Job Object, so terminating it also ends everything it
//! started (an agent CLI spawns shells and tools), and nothing survives the runner. A child is
//! assigned to its job right after it starts; a grandchild spawned in that instant escapes the
//! child's job but not the desktop's job around the runner.

use std::{
  collections::HashMap,
  ffi::OsStr,
  io,
  path::{Path, PathBuf},
  process::Stdio,
};

use orchester_ipc::{KillOnCloseJob, OutputStream, ProcessEvent, ProcessExit, ProcessId};
use thiserror::Error;
use tokio::{
  io::{AsyncBufReadExt, AsyncRead, BufReader},
  process::Child,
  sync::{mpsc, oneshot},
};

use crate::CREATE_NO_WINDOW;

/// Events buffered before output readers wait for the consumer; a slow consumer then slows the
/// child through its full pipe instead of growing memory.
const EVENT_BUFFER: usize = 1024;

#[derive(Debug, Error)]
pub enum SupervisorError {
  #[error("cannot start {program} in {working_dir}: {source}")]
  Spawn {
    program: PathBuf,
    working_dir: PathBuf,
    source: io::Error,
  },
  #[error("cannot contain {program} in a job object: {source}")]
  Job { program: PathBuf, source: io::Error },
  #[error("{0} is not running")]
  NotRunning(ProcessId),
}

/// Owns the runner's child processes. Dropping it terminates all of them.
pub struct Supervisor {
  events: mpsc::Sender<ProcessEvent>,
  next_id: u64,
  running: HashMap<ProcessId, oneshot::Sender<()>>,
}

impl Supervisor {
  /// A supervisor and the stream of its processes' events.
  pub fn new() -> (Self, mpsc::Receiver<ProcessEvent>) {
    let (events, receiver) = mpsc::channel(EVENT_BUFFER);
    let supervisor = Self {
      events,
      next_id: 1,
      running: HashMap::new(),
    };
    (supervisor, receiver)
  }

  /// Starts `program` with `args` in `working_dir`. Must be called within a Tokio runtime.
  pub fn spawn<I, S>(&mut self, program: &Path, args: I, working_dir: &Path) -> Result<ProcessId, SupervisorError>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
  {
    self.running.retain(|_, terminate| !terminate.is_closed());
    let child = tokio::process::Command::new(program)
      .args(args)
      .current_dir(working_dir)
      .stdin(Stdio::null())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .creation_flags(CREATE_NO_WINDOW)
      .kill_on_drop(true)
      .spawn()
      .map_err(|source| SupervisorError::Spawn {
        program: program.to_path_buf(),
        working_dir: working_dir.to_path_buf(),
        source,
      })?;
    let job = contain(&child).map_err(|source| SupervisorError::Job {
      program: program.to_path_buf(),
      source,
    })?;
    let id = ProcessId::new(self.next_id);
    self.next_id += 1;
    let (terminate, terminated) = oneshot::channel();
    self.running.insert(id, terminate);
    tokio::spawn(supervise(id, child, job, terminated, self.events.clone()));
    Ok(id)
  }

  /// Terminates the process and everything it started. Its exit is reported as
  /// [`ProcessExit::Terminated`] after its remaining output.
  pub fn terminate(&mut self, id: ProcessId) -> Result<(), SupervisorError> {
    let terminate = self.running.remove(&id).ok_or(SupervisorError::NotRunning(id))?;
    terminate.send(()).map_err(|()| SupervisorError::NotRunning(id))
  }
}

fn contain(child: &Child) -> io::Result<KillOnCloseJob> {
  let job = KillOnCloseJob::new()?;
  let handle = child
    .raw_handle()
    .ok_or_else(|| io::Error::other("the process exited before it could join its job"))?;
  job.assign(handle)?;
  Ok(job)
}

/// Runs until the child has exited and all its output is forwarded. A terminate request, or the
/// supervisor being dropped, closes the job and so kills the process tree.
async fn supervise(
  id: ProcessId,
  mut child: Child,
  job: KillOnCloseJob,
  terminated: oneshot::Receiver<()>,
  events: mpsc::Sender<ProcessEvent>,
) {
  let readers = [
    child
      .stdout
      .take()
      .map(|stdout| tokio::spawn(forward_lines(id, OutputStream::Stdout, stdout, events.clone()))),
    child
      .stderr
      .take()
      .map(|stderr| tokio::spawn(forward_lines(id, OutputStream::Stderr, stderr, events.clone()))),
  ];
  let exit = tokio::select! {
    status = child.wait() => match status {
      Ok(status) => ProcessExit::Exited { code: status.code() },
      Err(error) => ProcessExit::WaitFailed { reason: error.to_string() },
    },
    _ = terminated => ProcessExit::Terminated,
  };
  // Closing the job ends the child (when terminating) and anything it left running, which
  // also closes the output pipes those processes still hold.
  drop(job);
  if exit == ProcessExit::Terminated
    && let Err(error) = child.wait().await
  {
    tracing::warn!("{id}: cannot confirm termination: {error}");
  }
  for reader in readers.into_iter().flatten() {
    if let Err(error) = reader.await {
      tracing::warn!("{id}: output reader failed: {error}");
    }
  }
  // Nobody listens once the supervisor's receiver is gone; there is nothing left to report to.
  if events.send(ProcessEvent::Exited { process: id, exit }).await.is_err() {
    tracing::debug!("{id}: exit not reported, event receiver closed");
  }
}

async fn forward_lines(
  id: ProcessId,
  stream: OutputStream,
  output: impl AsyncRead + Unpin,
  events: mpsc::Sender<ProcessEvent>,
) {
  let mut reader = BufReader::new(output);
  let mut line = Vec::new();
  loop {
    line.clear();
    match reader.read_until(b'\n', &mut line).await {
      Ok(0) => return,
      Ok(_) => {}
      Err(error) => {
        tracing::warn!("{id}: cannot read {stream:?}: {error}");
        return;
      }
    }
    let text = String::from_utf8_lossy(&line).trim_end_matches(['\r', '\n']).to_owned();
    let event = ProcessEvent::Output {
      process: id,
      stream,
      line: text,
    };
    if events.send(event).await.is_err() {
      return;
    }
  }
}
