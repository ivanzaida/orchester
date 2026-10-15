use std::{
  io::{self, Read, Write},
  process::{Child, Stdio},
  sync::mpsc,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{command::Command, integration::local::ProcessJournal};

use super::{LocalError, LocalGitError, family::Family, process::GitOutput};
const MAX_BYTES: u64 = 16 * 1024 * 1024;

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
  let stdout = child.stdout.take().ok_or(LocalGitError::Process)?;
  let stderr = child.stderr.take().ok_or(LocalGitError::Process)?;
  let output = worker(move || bounded(stdout, capture));
  let errors = worker(move || {
    io::copy(&mut { stderr }, &mut io::sink())
      .map(|_| ())
      .map_err(|_| LocalGitError::Io)
  });
  let writer = input.map(|input| {
    let bytes = input.to_owned();
    let pipe = child.stdin.take();
    worker(move || match pipe {
      Some(mut pipe) => match pipe.write_all(&bytes) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(_) => Err(LocalGitError::Io),
      },
      None => Err(LocalGitError::Process),
    })
  });
  let status = finish_family(&mut child, &family, timeout);
  let deadline = Instant::now() + Duration::from_secs(2);
  let bytes = receive(&output, deadline)?;
  receive(&errors, deadline)?;
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
  })
}

fn spawn(
  command: &Command,
  input: bool,
  family: &Family,
  journal: Option<(&dyn ProcessJournal, u64)>,
) -> Result<Child, LocalGitError> {
  let mut process = crate::prepare(command).map_err(|_| LocalGitError::Process)?;
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
      await_empty(family)?;
      child.wait().map_err(|_| LocalError::ProcessUncertain)?;
      return Err(LocalGitError::Process);
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
      .map_err(|_| LocalGitError::Io)?;
  }
  io::copy(&mut pipe, &mut io::sink()).map_err(|_| LocalGitError::Io)?;
  if bytes.len() as u64 > MAX_BYTES {
    return Err(LocalError::Unsupported.into());
  }
  Ok(bytes)
}
