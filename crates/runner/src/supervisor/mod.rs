//! Starts agent CLIs and other child processes, streams their output line by line and ends
//! them. Each child is contained (see [`contain`]) so that ending it also ends everything it
//! started: an agent CLI spawns shells and tools.
//!
//! On Windows a child joins its Job Object right after it starts; a grandchild spawned in that
//! instant escapes the child's job but not the desktop's job around the runner. On macOS the
//! child leads its process group from before it runs, so there is no such instant.

mod contain;

use std::{io, path::PathBuf};

use orchester_core::{
  command::{Command, CommandError},
  connection::stream::OutputStream,
};
use thiserror::Error;
use tokio::{
  io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader},
  process::{Child, ChildStdin},
  sync::{mpsc, oneshot},
};

use self::contain::Containment;

/// Events buffered before the output readers wait for the consumer; a slow consumer then slows
/// the child through its full pipe instead of growing memory.
const EVENT_BUFFER: usize = 1024;

#[derive(Debug, Error)]
pub enum SupervisorError {
  #[error("cannot start {program}: {source}")]
  Launch { program: PathBuf, source: CommandError },
  #[error("cannot start {program}: {source}")]
  Spawn { program: PathBuf, source: io::Error },
  #[error("cannot contain {program}: {source}")]
  Contain { program: PathBuf, source: io::Error },
}

/// How a supervised process ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessExit {
  /// The process ended on its own.
  Exited { code: Option<i32> },
  /// The supervisor ended the process and everything it started.
  Terminated,
  /// The operating system could not report how the process ended; it has been ended.
  WaitFailed { reason: String },
}

/// What a supervised process did. All output is reported before the exit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessEvent {
  /// One line, without its line terminator; bytes that are not UTF-8 are replaced.
  Output {
    stream: OutputStream,
    line: String,
  },
  Exited(ProcessExit),
}

/// A running child. Dropping it ends the child and everything it started.
pub struct Supervised {
  events: mpsc::Receiver<ProcessEvent>,
  terminate: Option<oneshot::Sender<()>>,
  input: Option<mpsc::Sender<(String, oneshot::Sender<io::Result<()>>)>>,
}

impl Supervised {
  /// Starts `command` as [`orchester_exec::prepare`] builds it, writing its standard input, if
  /// any, then closing it; an interactive one gets its first line, if any, and stays open. Must be
  /// called within a Tokio runtime.
  pub fn start(command: &Command) -> Result<Self, SupervisorError> {
    let program = command.program().to_path_buf();
    let process = orchester_exec::prepare(command).map_err(|source| SupervisorError::Launch {
      program: program.clone(),
      source,
    })?;
    let mut child = tokio::process::Command::from(process)
      .kill_on_drop(true)
      .spawn()
      .map_err(|source| SupervisorError::Spawn {
        program: program.clone(),
        source,
      })?;
    let containment = Containment::of(&child).map_err(|source| SupervisorError::Contain { program, source })?;
    let input = if command.interactive_stdin() {
      let stdin = child.stdin.take().ok_or_else(|| SupervisorError::Spawn {
        program: command.program().to_path_buf(),
        source: io::Error::new(io::ErrorKind::BrokenPipe, "interactive child has no stdin"),
      })?;
      let (sender, receiver) = mpsc::channel(32);
      if let Some(first) = command.stdin() {
        let (ack, written) = oneshot::channel();
        sender
          .try_send((first.to_owned(), ack))
          .expect("a new input channel has room for its first line");
        // A first line the child does not take shows in how it ends.
        tokio::spawn(async move {
          if let Ok(Err(error)) = written.await {
            tracing::warn!(%error, "cannot write the program's first input line");
          }
        });
      }
      tokio::spawn(write_lines(stdin, receiver));
      Some(sender)
    } else {
      if let (Some(stdin), Some(input)) = (child.stdin.take(), command.stdin()) {
        tokio::spawn(write_input(stdin, input.to_owned()));
      }
      None
    };
    let (sender, events) = mpsc::channel(EVENT_BUFFER);
    let (terminate, terminated) = oneshot::channel();
    tokio::spawn(supervise(child, containment, terminated, sender));
    Ok(Self {
      events,
      terminate: Some(terminate),
      input,
    })
  }

  /// The next event; `None` after the exit was reported.
  pub async fn next(&mut self) -> Option<ProcessEvent> {
    self.events.recv().await
  }

  /// Sends one JSON-RPC message to an interactive child, retaining its stdin for later replies.
  pub async fn send_line(&self, line: String) -> io::Result<()> {
    let input = self
      .input
      .as_ref()
      .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "child stdin is not interactive"))?;
    let (ack, done) = oneshot::channel();
    input
      .send((line, ack))
      .await
      .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "child stdin closed"))?;
    done
      .await
      .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "child stdin writer ended"))?
  }

  /// Closes an interactive child's standard input once the lines sent before have been written, so
  /// that it sees the end of its input; later lines are refused.
  pub fn close_input(&mut self) {
    self.input = None;
  }

  /// Whether an interactive child's standard input is still open for lines.
  pub const fn takes_input(&self) -> bool {
    self.input.is_some()
  }

  /// Ends the process and everything it started. Its exit is reported as
  /// [`ProcessExit::Terminated`] after its remaining output.
  pub fn terminate(&mut self) {
    if let Some(terminate) = self.terminate.take() {
      // The process may have exited already; there is nothing left to end then.
      let _ = terminate.send(());
    }
  }
}

async fn write_input(mut stdin: ChildStdin, input: String) {
  if let Err(error) = stdin.write_all(input.as_bytes()).await {
    // The program exited without reading all of its input; its exit says how it went.
    tracing::warn!(%error, "cannot write the program's input");
  }
  // Dropping `stdin` closes it, so the program sees the end of its input.
}

async fn write_lines(mut stdin: ChildStdin, mut lines: mpsc::Receiver<(String, oneshot::Sender<io::Result<()>>)>) {
  while let Some((line, ack)) = lines.recv().await {
    let result = async {
      stdin.write_all(line.as_bytes()).await?;
      stdin.write_all(b"\n").await?;
      stdin.flush().await
    }
    .await;
    let failed = result.is_err();
    // The caller may have ended the invocation before the pipe write finished.
    if ack.send(result).is_err() {
      tracing::debug!("the interactive child no longer waits for its input write");
    }
    if failed {
      break;
    }
  }
}

/// Runs until the child has exited and all its output is forwarded. A terminate request, or
/// the handle being dropped, ends the child's process tree.
async fn supervise(
  mut child: Child,
  containment: Containment,
  terminated: oneshot::Receiver<()>,
  events: mpsc::Sender<ProcessEvent>,
) {
  let readers = [
    child
      .stdout
      .take()
      .map(|stdout| tokio::spawn(forward_lines(OutputStream::Stdout, stdout, events.clone()))),
    child
      .stderr
      .take()
      .map(|stderr| tokio::spawn(forward_lines(OutputStream::Stderr, stderr, events.clone()))),
  ];
  let exit = tokio::select! {
    status = child.wait() => match status {
      Ok(status) => ProcessExit::Exited { code: status.code() },
      Err(error) => ProcessExit::WaitFailed { reason: error.to_string() },
    },
    _ = terminated => ProcessExit::Terminated,
  };
  // Ends the child (when terminating) and anything it left running, which also closes the
  // output pipes those processes still hold.
  containment.end(&mut child).await;
  for reader in readers.into_iter().flatten() {
    if let Err(error) = reader.await {
      tracing::warn!("output reader failed: {error}");
    }
  }
  // Nobody listens once the handle is gone; there is nothing left to report to.
  if events.send(ProcessEvent::Exited(exit)).await.is_err() {
    tracing::debug!("exit not reported, the handle was dropped");
  }
}

async fn forward_lines(stream: OutputStream, output: impl AsyncRead + Unpin, events: mpsc::Sender<ProcessEvent>) {
  let mut reader = BufReader::new(output);
  let mut line = Vec::new();
  loop {
    line.clear();
    match reader.read_until(b'\n', &mut line).await {
      Ok(0) => return,
      Ok(_) => {}
      Err(error) => {
        tracing::warn!("cannot read {stream:?}: {error}");
        return;
      }
    }
    let line = String::from_utf8_lossy(&line).trim_end_matches(['\r', '\n']).to_owned();
    if events.send(ProcessEvent::Output { stream, line }).await.is_err() {
      return;
    }
  }
}
