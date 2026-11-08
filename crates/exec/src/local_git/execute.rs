use std::{
  io::{self, Read, Write},
  process::{Child, Stdio},
  sync::mpsc,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  command::Command,
  integration::{IoCause, local::ProcessJournal},
};

use super::{LocalError, LocalGitError, family::Family, process::GitOutput};
const MAX_BYTES: u64 = 16 * 1024 * 1024;
/// How long a job may still hold processes after its program exited before they count as left behind. A busy
/// machine tears an exited process down late, which is no writer left running.
const SETTLE: Duration = Duration::from_secs(2);
/// How long a stopped job and the pipes of the program it held may take to end. Once the job is empty nothing is left
/// to write to them, so only a machine too busy to run the readers makes this run out.
const DRAIN: Duration = Duration::from_secs(15);
/// The end of Git's error output a refusal keeps.
const ERROR_TAIL: usize = 4 * 1024;

/// What a program's outcome is when a process of its family outlives it and has to be stopped.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Leftovers {
  /// Unknown: the process may have written what the program was run for, so the work is not called done.
  Uncertain,
  /// The program only reads or writes what a retry replaces; its own exit status stands once the family is empty.
  Stopped,
}

/// A program's receipt in the journal: the journal and the receipt's sequence number.
type Receipt<'a> = (&'a dyn ProcessJournal, u64);

pub(super) fn invoke(
  command: &Command,
  input: Option<&[u8]>,
  (timeout, leftovers): (Duration, Leftovers),
  capture: bool,
  journal: Option<&dyn ProcessJournal>,
) -> Result<GitOutput, LocalGitError> {
  let family = Family::create()?;
  let receipt = journal
    .map(|journal| journal.begin().map(|sequence| (journal, sequence)))
    .transpose()?;
  let run = run(command, input, (timeout, leftovers, capture), (&family, receipt));
  close_receipt(&family, receipt, run)
}

fn run(
  command: &Command,
  input: Option<&[u8]>,
  (timeout, leftovers, capture): (Duration, Leftovers, bool),
  (family, receipt): (&Family, Option<Receipt<'_>>),
) -> Result<GitOutput, LocalGitError> {
  let mut child = spawn(command, input.is_some(), family, receipt)?;
  let missing_pipe = || LocalGitError::process(io::ErrorKind::BrokenPipe);
  let stdout = child.stdout.take().ok_or_else(missing_pipe)?;
  let stderr = child.stderr.take().ok_or_else(missing_pipe)?;
  let output = worker(move || bounded(stdout, capture));
  let errors = worker(move || tail(stderr));
  let writer = input.map(|input| {
    let bytes = input.to_owned();
    let pipe = child.stdin.take();
    worker(move || match pipe {
      Some(mut pipe) => match pipe.write_all(&bytes) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(LocalGitError::io(&error)),
      },
      None => Err(missing_pipe()),
    })
  });
  let status = finish_family(&mut child, family, (timeout, leftovers));
  let deadline = Instant::now() + DRAIN;
  let bytes = receive(&output, deadline)?;
  let errors = receive(&errors, deadline)?;
  if let Some(writer) = writer {
    receive(&writer, deadline)?;
  }
  if !family.empty()? {
    return Err(LocalError::ProcessUncertain.into());
  }
  Ok(GitOutput {
    code: status?.code(),
    bytes,
    errors,
  })
}

/// Ends the program's receipt once its job is proved empty, whatever its outcome `run` was: the receipt says whether
/// anything of the program may still run, and one left unfinished is refused by every later reconciliation
/// ([`super::family::reconcile_stopped`]) for as long as this process lives. What is left of the job is stopped here.
fn close_receipt(
  family: &Family,
  receipt: Option<Receipt<'_>>,
  run: Result<GitOutput, LocalGitError>,
) -> Result<GitOutput, LocalGitError> {
  let Some((journal, sequence)) = receipt else {
    return run;
  };
  let empty = family.empty().is_ok_and(|empty| empty) || (family.stop().is_ok() && await_empty(family).is_ok());
  if !empty {
    return run;
  }
  match (journal.finished(sequence), run) {
    (Ok(()), run) => run,
    (Err(error), Ok(_)) => Err(error.into()),
    (Err(error), run) => {
      tracing::warn!(%error, "cannot end the receipt of a Git program that failed");
      run
    }
  }
}

fn spawn(
  command: &Command,
  input: bool,
  family: &Family,
  receipt: Option<Receipt<'_>>,
) -> Result<Child, LocalGitError> {
  let mut process = crate::prepare(command).map_err(|error| {
    let kind = match error {
      orchester_core::command::CommandError::NotFound => io::ErrorKind::NotFound,
      _ => io::ErrorKind::Other,
    };
    LocalGitError::Process(IoCause::new(&io::Error::new(kind, error.to_string())))
  })?;
  process.stdin(if input { Stdio::piped() } else { Stdio::null() });
  let child = family.spawn(&mut process)?;
  let recorded = family.identity(&child).and_then(|identity| {
    if let Some((journal, sequence)) = receipt {
      journal.started(sequence, Some(&identity))?;
    }
    family.resume(&child)
  });
  if let Err(error) = recorded {
    family.stop()?;
    return Err(error);
  }
  Ok(child)
}

fn finish_family(
  child: &mut Child,
  family: &Family,
  (timeout, leftovers): (Duration, Leftovers),
) -> Result<std::process::ExitStatus, LocalGitError> {
  let deadline = Instant::now() + timeout;
  loop {
    let status = child.try_wait().map_err(|_| LocalError::ProcessUncertain)?;
    if let Some(status) = status {
      let settle = Instant::now() + SETTLE;
      loop {
        // A delayed wake must observe the job before deciding that the settle bound expired.
        if family.empty()? {
          return Ok(status);
        }
        if Instant::now() >= settle {
          break;
        }
        thread::sleep(Duration::from_millis(10));
      }
      // A detached writer remains part of our job. Stop it, but do not call its interrupted work a success.
      tracing::warn!(settle = ?SETTLE, "a process of a Git program's job outlived the program and was stopped");
      family.stop()?;
      await_empty(family)?;
      return match leftovers {
        Leftovers::Uncertain => Err(LocalError::ProcessUncertain.into()),
        Leftovers::Stopped => Ok(status),
      };
    }
    if Instant::now() >= deadline {
      family.stop()?;
      // Reaped first: a process group counts its leader until then, which a job does not.
      child.wait().map_err(|_| LocalError::ProcessUncertain)?;
      await_empty(family)?;
      return Err(LocalGitError::process(io::ErrorKind::TimedOut));
    }
    thread::sleep(Duration::from_millis(10));
  }
}
fn await_empty(family: &Family) -> Result<(), LocalGitError> {
  let deadline = Instant::now() + DRAIN;
  while !family.empty()? {
    if Instant::now() >= deadline {
      return Err(LocalError::ProcessUncertain.into());
    }
    thread::sleep(Duration::from_millis(10));
  }
  Ok(())
}
fn worker<T: Send + 'static>(
  work: impl FnOnce() -> Result<T, LocalGitError> + Send + 'static,
) -> mpsc::Receiver<Result<T, LocalGitError>> {
  let (send, receive) = mpsc::channel();
  thread::spawn(move || {
    // The receiver may have returned an explicit uncertain outcome. Losing delivery cannot claim success.
    if send.send(work()).is_err() {}
  });
  receive
}
fn receive<T>(pipe: &mpsc::Receiver<Result<T, LocalGitError>>, deadline: Instant) -> Result<T, LocalGitError> {
  pipe
    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
    .map_err(|_| {
      tracing::warn!(drain = ?DRAIN, "the pipes of a Git program were not closed in time");
      LocalError::ProcessUncertain
    })?
}
fn bounded(mut pipe: impl Read, capture: bool) -> Result<Vec<u8>, LocalGitError> {
  let mut bytes = Vec::new();
  if capture {
    pipe
      .by_ref()
      .take(MAX_BYTES + 1)
      .read_to_end(&mut bytes)
      .map_err(|error| LocalGitError::io(&error))?;
  }
  io::copy(&mut pipe, &mut io::sink()).map_err(|error| LocalGitError::io(&error))?;
  if bytes.len() as u64 > MAX_BYTES {
    return Err(LocalError::Unsupported.into());
  }
  Ok(bytes)
}

/// The last [`ERROR_TAIL`] bytes `pipe` yields, read to its end.
fn tail(mut pipe: impl Read) -> Result<Vec<u8>, LocalGitError> {
  let mut kept = Vec::new();
  let mut buffer = [0; 8 * 1024];
  loop {
    let count = match pipe.read(&mut buffer) {
      Ok(0) => return Ok(kept),
      Ok(count) => count,
      Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
      Err(error) => return Err(LocalGitError::io(&error)),
    };
    kept.extend_from_slice(&buffer[..count]);
    let excess = kept.len().saturating_sub(ERROR_TAIL);
    kept.drain(..excess);
  }
}
