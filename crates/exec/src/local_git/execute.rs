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
/// The end of Git's error output a refusal keeps.
const ERROR_TAIL: usize = 4 * 1024;

pub(super) fn invoke(
  command: &Command,
  input: Option<&[u8]>,
  timeout: Duration,
  capture: bool,
  journal: Option<&dyn ProcessJournal>,
) -> Result<GitOutput, LocalGitError> {
  let family = Family::create()?;
  let sequence = journal.map(ProcessJournal::begin).transpose()?;
  let mut child = spawn(command, input.is_some(), &family, journal.zip(sequence))?;
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
  let status = finish_family(&mut child, &family, timeout);
  let deadline = Instant::now() + Duration::from_secs(2);
  let bytes = receive(&output, deadline)?;
  let errors = receive(&errors, deadline)?;
  if let Some(writer) = writer {
    receive(&writer, deadline)?;
  }
  if !family.empty()? {
    return Err(LocalError::ProcessUncertain.into());
  }
  if let Some((journal, sequence)) = journal.zip(sequence) {
    journal.finished(sequence)?;
  }
  Ok(GitOutput {
    code: status?.code(),
    bytes,
    errors,
  })
}

fn spawn(
  command: &Command,
  input: bool,
  family: &Family,
  journal: Option<(&dyn ProcessJournal, u64)>,
) -> Result<Child, LocalGitError> {
  let mut process = crate::prepare(command).map_err(|error| {
    let kind = match error {
      orchester_core::command::CommandError::NotFound => io::ErrorKind::NotFound,
      _ => io::ErrorKind::Other,
    };
    LocalGitError::Process(IoCause::new(&io::Error::new(kind, error.to_string())))
  })?;
  process.stdin(if input { Stdio::piped() } else { Stdio::null() });
  let child = match family.spawn(&mut process) {
    Ok(child) => child,
    Err(error) => {
      if family.empty()?
        && let Some((journal, sequence)) = journal
      {
        journal.finished(sequence)?;
      }
      return Err(error);
    }
  };
  let recorded = family.identity(&child).and_then(|identity| {
    if let Some((journal, sequence)) = journal {
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
  timeout: Duration,
) -> Result<std::process::ExitStatus, LocalGitError> {
  let deadline = Instant::now() + timeout;
  loop {
    let status = child.try_wait().map_err(|_| LocalError::ProcessUncertain)?;
    if let Some(status) = status {
      let settle = Instant::now() + Duration::from_millis(200);
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
      family.stop()?;
      await_empty(family)?;
      return Err(LocalError::ProcessUncertain.into());
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
  let deadline = Instant::now() + Duration::from_secs(2);
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
    .map_err(|_| LocalError::ProcessUncertain)?
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
