//! A program run to completion whose output is read as it comes and kept within a cap: at most `limit`
//! bytes of each stream, its start or its end, the rest only counted. Orchester's tools run this way
//! (`orchester_core::orchester_tools`), so an agent that makes git print a huge blob never makes the
//! runner hold it.

use std::{
  collections::VecDeque,
  io::{self, Read},
  thread,
  time::Duration,
};

use orchester_core::{
  command::{Command, CommandError},
  orchester_tools::{OutputKept, ProgramOutput},
};

use super::{joined_input, spawn, wait, write_all};

/// How much is read from a pipe at a time.
const CHUNK: usize = 8 * 1024;

/// Runs `command`, stopping it with everything it started when it runs longer than `timeout`, and keeps
/// at most `limit` bytes of each stream at the `kept` end.
pub fn run_capped(
  command: &Command,
  timeout: Duration,
  (limit, kept): (usize, OutputKept),
) -> Result<ProgramOutput, CommandError> {
  let mut child = spawn(command)?;
  let stdin = command
    .stdin()
    .map(|input| write_all(child.stdin.take(), input.to_owned()));
  let stdout = capture(child.stdout.take(), limit, kept);
  let stderr = capture(child.stderr.take(), limit, kept);
  let status = wait(&mut child, command, timeout)?;
  if let Some(writer) = stdin {
    joined_input(writer)?;
  }
  let (stdout, stdout_left_out) = joined(stdout)?;
  let (stderr, stderr_left_out) = joined(stderr)?;
  Ok(ProgramOutput {
    code: status.code(),
    stdout,
    stderr,
    left_out: stdout_left_out + stderr_left_out,
  })
}

/// Reads `pipe` to its end on a thread of its own, keeping at most `limit` bytes at the `kept` end, and
/// how many bytes it did not keep.
fn capture(
  pipe: Option<impl Read + Send + 'static>,
  limit: usize,
  kept: OutputKept,
) -> thread::JoinHandle<io::Result<(Vec<u8>, u64)>> {
  thread::spawn(move || {
    let mut held = VecDeque::with_capacity(limit.min(CHUNK * 8));
    let mut left_out = 0_u64;
    let Some(mut pipe) = pipe else {
      return Ok((Vec::new(), 0));
    };
    let mut chunk = vec![0; CHUNK];
    loop {
      let read = match pipe.read(&mut chunk) {
        Ok(0) => break,
        Ok(read) => read,
        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
        Err(error) => return Err(error),
      };
      let bytes = &chunk[..read];
      match kept {
        OutputKept::Head => {
          let room = limit.saturating_sub(held.len()).min(bytes.len());
          held.extend(&bytes[..room]);
          left_out += (bytes.len() - room) as u64;
        }
        OutputKept::Tail => {
          held.extend(bytes);
          let excess = held.len().saturating_sub(limit);
          held.drain(..excess);
          left_out += excess as u64;
        }
      }
    }
    Ok((held.into(), left_out))
  })
}

fn joined(reader: thread::JoinHandle<io::Result<(Vec<u8>, u64)>>) -> Result<(String, u64), CommandError> {
  let (bytes, left_out) = reader
    .join()
    .map_err(|_| CommandError::Failed("the output reader panicked".into()))?
    .map_err(|error| CommandError::Failed(error.to_string()))?;
  Ok((String::from_utf8_lossy(&bytes).into_owned(), left_out))
}
