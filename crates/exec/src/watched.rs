//! Programs run to completion under a watchdog on their silence, for the git commands of a merge: a git on a disk that
//! stops answering would otherwise hold its merge for ever. One that writes nothing, to its output or its errors, for
//! the silence it is given is stopped with everything it started — its private job on Windows, its process group on
//! macOS ([`crate::local_git::family`]) — and ends as [`CommandError::Silent`]. A program it started that keeps the
//! pipes open after it exited is watched the same way.

use std::{
  io::{self, Read},
  process::{Child, ExitStatus},
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use orchester_core::command::{Command, CommandError, CommandOutput, CommandRunner};

use crate::{joined_input, local_git::family::Family, prepare, write_all};

const POLL_INTERVAL: Duration = Duration::from_millis(50);
/// How long the output of a stopped program may take to end once everything it started is gone.
const DRAIN: Duration = Duration::from_secs(2);

/// Runs programs as child processes, each with everything it starts, stopping one that writes nothing for `silence`
/// or does not finish within `timeout`.
#[derive(Clone, Copy, Debug)]
pub struct WatchedRunner {
  timeout: Duration,
  silence: Duration,
}

impl WatchedRunner {
  pub const fn new(timeout: Duration, silence: Duration) -> Self {
    Self { timeout, silence }
  }
}

/// When a program last wrote, in milliseconds after it started.
struct Heard {
  start: Instant,
  last: AtomicU64,
}

impl Heard {
  fn now(&self) {
    let elapsed = u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX);
    self.last.store(elapsed, Ordering::Relaxed);
  }

  fn silent_for(&self) -> Duration {
    self
      .start
      .elapsed()
      .saturating_sub(Duration::from_millis(self.last.load(Ordering::Relaxed)))
  }
}

type Reader = thread::JoinHandle<io::Result<String>>;

/// How the watch of a program ended.
enum Ended {
  Exited(ExitStatus),
  Stopped(CommandError),
}

impl CommandRunner for WatchedRunner {
  fn run(&self, command: &Command) -> Result<CommandOutput, CommandError> {
    let family = Family::create().map_err(|error| CommandError::NotStarted(error.to_string()))?;
    let mut process = prepare(command)?;
    let mut child = family.spawn(&mut process).map_err(|error| match error {
      crate::local_git::LocalGitError::Process(cause) if cause.kind() == io::ErrorKind::NotFound => {
        CommandError::NotFound
      }
      error => CommandError::NotStarted(error.to_string()),
    })?;
    if let Err(error) = family.resume(&child) {
      stop(&family, &mut child);
      return Err(CommandError::NotStarted(error.to_string()));
    }
    let heard = Arc::new(Heard {
      start: Instant::now(),
      last: AtomicU64::new(0),
    });
    let stdin = command
      .stdin()
      .map(|input| write_all(child.stdin.take(), input.to_owned()));
    let stdout = listen(child.stdout.take(), heard.clone());
    let stderr = listen(child.stderr.take(), heard.clone());
    let status = match self.watch((&mut child, &family), [&stdout, &stderr], &heard) {
      Ended::Exited(status) => status,
      Ended::Stopped(error) => {
        tracing::warn!(%error, program = %command.program().display(), "stopped a program with everything it started");
        return Err(error);
      }
    };
    if let Some(writer) = stdin {
      joined_input(writer)?;
    }
    Ok(CommandOutput {
      code: status.code(),
      stdout: joined(stdout)?,
      stderr: joined(stderr)?,
    })
  }
}

impl WatchedRunner {
  /// Waits until `child` exited and its output ended, stopping its family when it is silent or late.
  fn watch(&self, (child, family): (&mut Child, &Family), readers: [&Reader; 2], heard: &Heard) -> Ended {
    let mut exited = None;
    loop {
      if exited.is_none() {
        match child.try_wait() {
          Ok(status) => exited = status,
          Err(error) => {
            stop(family, child);
            return Ended::Stopped(CommandError::Failed(error.to_string()));
          }
        }
      }
      let drained = readers.iter().all(|reader| reader.is_finished());
      if let (Some(status), true) = (exited, drained) {
        return Ended::Exited(status);
      }
      let late = heard.start.elapsed() >= self.timeout;
      if late || heard.silent_for() >= self.silence {
        stop(family, child);
        if let (Some(status), true) = (exited, drain(readers)) {
          // Git ended, and only what it left behind holding its output did not.
          return Ended::Exited(status);
        }
        return Ended::Stopped(if late {
          CommandError::TimedOut
        } else {
          CommandError::Silent { after: self.silence }
        });
      }
      thread::sleep(POLL_INTERVAL);
    }
  }
}

/// Stops `child` with everything it started, and reaps it. Errors are logged: the caller reports the program as
/// stopped either way, and a family that cannot be stopped holds nothing of the caller's.
fn stop(family: &Family, child: &mut Child) {
  if let Err(error) = family.stop() {
    tracing::warn!(%error, "cannot stop a program with everything it started");
  }
  if let Err(error) = child.wait() {
    tracing::warn!(%error, "cannot reap a stopped program");
  }
}

/// Whether the readers ended within [`DRAIN`].
fn drain(readers: [&Reader; 2]) -> bool {
  let deadline = Instant::now() + DRAIN;
  while Instant::now() < deadline {
    if readers.iter().all(|reader| reader.is_finished()) {
      return true;
    }
    thread::sleep(POLL_INTERVAL);
  }
  false
}

/// Reads `pipe` to its end on a thread of its own, telling `heard` of every write.
fn listen(pipe: Option<impl Read + Send + 'static>, heard: Arc<Heard>) -> Reader {
  thread::spawn(move || {
    let mut bytes = Vec::new();
    let Some(mut pipe) = pipe else {
      return Ok(String::new());
    };
    let mut chunk = [0; 8 * 1024];
    loop {
      match pipe.read(&mut chunk) {
        Ok(0) => break,
        Ok(read) => {
          heard.now();
          bytes.extend_from_slice(&chunk[..read]);
        }
        Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
        Err(error) => return Err(error),
      }
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
  })
}

fn joined(reader: Reader) -> Result<String, CommandError> {
  reader
    .join()
    .map_err(|_| CommandError::Failed("the output reader panicked".into()))?
    .map_err(|error| CommandError::Failed(error.to_string()))
}
