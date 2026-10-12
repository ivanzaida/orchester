use std::{
  io::{self, Read, Write},
  process::{Child, Stdio},
  thread,
  time::Duration,
};

use orchester_core::{command::Command, integration::local::ProcessJournal};

use super::{LocalError, LocalGitError, process::GitOutput};

const MAX_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn invoke(
  command: &Command,
  input: Option<&[u8]>,
  timeout: Duration,
  capture: bool,
  journal: Option<&dyn ProcessJournal>,
) -> Result<GitOutput, LocalGitError> {
  let sequence = journal.map(ProcessJournal::begin).transpose()?;
  let mut child = spawn(command, input.is_some(), journal.zip(sequence))?;
  let stdout = child.stdout.take().ok_or(LocalGitError::Process)?;
  let stderr = child.stderr.take().ok_or(LocalGitError::Process)?;
  let reader = thread::spawn(move || bounded(stdout, capture));
  let errors = thread::spawn(move || io::copy(&mut { stderr }, &mut io::sink()));
  let writer = input.map(|input| {
    let bytes = input.to_owned();
    let pipe = child.stdin.take();
    thread::spawn(move || match pipe {
      Some(mut pipe) => pipe.write_all(&bytes),
      None => Err(io::Error::other("no input pipe")),
    })
  });
  let result = crate::wait(&mut child, command, timeout);
  if let Some(writer) = writer {
    match writer.join().map_err(|_| LocalGitError::Process)? {
      Ok(()) => {}
      Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {}
      Err(_) => return Err(LocalGitError::Process),
    }
  }
  errors
    .join()
    .map_err(|_| LocalGitError::Process)?
    .map_err(|_| LocalGitError::Process)?;
  let bytes = reader.join().map_err(|_| LocalGitError::Process)?;
  if let Some((journal, sequence)) = journal.zip(sequence) {
    // wait has observed exit, or stopped this owned child on timeout. No other PID is signalled.
    if child.try_wait().map_err(|_| LocalGitError::Process)?.is_some() {
      journal.finished(sequence)?;
    } else {
      return Err(LocalError::Reconcile.into());
    }
  }
  Ok(GitOutput {
    code: result.map_err(|_| LocalGitError::Process)?.code(),
    bytes: bytes?,
  })
}

fn spawn(command: &Command, input: bool, journal: Option<(&dyn ProcessJournal, u64)>) -> Result<Child, LocalGitError> {
  let mut process = crate::prepare(command).map_err(|_| LocalGitError::Process)?;
  process.stdin(if input { Stdio::piped() } else { Stdio::null() });
  let mut child = match process.spawn() {
    Ok(child) => child,
    Err(_) => {
      if let Some((journal, sequence)) = journal {
        journal.finished(sequence)?;
      }
      return Err(LocalGitError::Process);
    }
  };
  if let Some((journal, sequence)) = journal {
    let recorded = crate::run_processes::identity::capture(child.id())
      .map_err(|_| LocalError::Reconcile)
      .and_then(|identity| journal.started(sequence, identity.as_ref()));
    if let Err(error) = recorded {
      crate::stop(&mut child).map_err(|_| LocalGitError::Process)?;
      child.wait().map_err(|_| LocalGitError::Process)?;
      journal.finished(sequence)?;
      return Err(error.into());
    }
  }
  Ok(child)
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
