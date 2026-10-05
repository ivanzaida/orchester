//! Running ripgrep for `list_files` and `search` (`docs/architecture/native-agent.md` §4.2, §4.3): with an
//! argument vector Orchester builds, `--no-config` so that no configuration file adds options such as
//! `--pre`, started as Orchester starts every program, and read line by line until the caller has
//! enough; then it is stopped. A run that takes too long is stopped as well.

use std::{
  io::{self, BufRead, BufReader, Read},
  path::Path,
  process::Child,
  sync::mpsc,
  thread,
  time::Duration,
};

use orchester_core::command::Command;

use super::FileError;

/// How long one ripgrep run may take.
const RIPGREP_TIMEOUT: Duration = Duration::from_secs(60);
/// The most of ripgrep's error output kept.
const MAX_STDERR: usize = 4 * 1024;

/// The most bytes of one output line kept; the rest of a longer line is skipped. ripgrep cuts matched
/// lines itself (`--max-columns`), so only a very long path reaches this.
const MAX_LINE: usize = 16 * 1024;

/// How a run ended.
#[derive(Debug)]
pub(super) struct Finished {
  /// The exit code; `None` when the run was stopped.
  pub code: Option<i32>,
  pub stderr: String,
  /// The caller had enough before ripgrep was done.
  pub stopped: bool,
}

/// Whether the caller wants more lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Want {
  More,
  Enough,
}

/// Runs `program` with `args` in `folder`, handing each line of its output to `line`.
pub(super) fn run(
  program: &Path,
  args: &[String],
  folder: &Path,
  mut line: impl FnMut(&str) -> Want,
) -> Result<Finished, FileError> {
  let mut all = vec!["--no-config".to_owned()];
  all.extend(args.iter().cloned());
  let command = Command::new(program, all).with_current_dir(folder);
  let mut child = orchester_exec::prepare(&command)
    .map_err(|error| FileError::Ripgrep(error.to_string()))?
    .spawn()
    .map_err(|error| FileError::Ripgrep(error.to_string()))?;
  let stdout = child.stdout.take();
  let stderr = child.stderr.take().map(drain);
  let (stop, stopping) = mpsc::channel();
  let watchdog = thread::spawn(move || watch(child, &stopping));
  let mut stopped = false;
  if let Some(stdout) = stdout {
    let mut reader = BufReader::new(stdout);
    let mut bytes = Vec::new();
    // A failed read ends the output like its end does; the exit code then tells what happened.
    while let Ok(true) = read_line(&mut reader, &mut bytes) {
      let text = String::from_utf8_lossy(&bytes);
      if line(text.trim_end_matches(['\n', '\r'])) == Want::Enough {
        stopped = true;
        break;
      }
    }
  }
  // The watchdog ends after this message or its timeout, whichever comes first; a closed channel
  // only means it already ended.
  stop.send(stopped).ok();
  let ended = watchdog
    .join()
    .map_err(|_| FileError::Ripgrep("the thread waiting for ripgrep panicked".to_owned()))?;
  // Without the error output (its thread panicked) the exit code still says whether the run failed.
  let stderr = stderr.and_then(|reader| reader.join().ok()).unwrap_or_default();
  match ended {
    Ended::Exited(code) => Ok(Finished { code, stderr, stopped }),
    Ended::Stopped => Ok(Finished {
      code: None,
      stderr,
      stopped: true,
    }),
    Ended::TimedOut => Err(FileError::Ripgrep(format!(
      "it did not finish within {} s",
      RIPGREP_TIMEOUT.as_secs()
    ))),
    Ended::Failed(error) => Err(FileError::Ripgrep(error)),
  }
}

enum Ended {
  Exited(Option<i32>),
  Stopped,
  TimedOut,
  Failed(String),
}

/// Waits for the reader to finish (`false`) or to stop early (`true`), or for the timeout; stops the
/// program in the last two cases.
fn watch(mut child: Child, stopping: &mpsc::Receiver<bool>) -> Ended {
  let stop = match stopping.recv_timeout(RIPGREP_TIMEOUT) {
    Ok(false) => None,
    Ok(true) => Some(Ended::Stopped),
    Err(_) => Some(Ended::TimedOut),
  };
  if stop.is_some() {
    // The program may have exited on its own meanwhile; waiting below reaps it either way.
    child.kill().ok();
  }
  match (child.wait(), stop) {
    (Ok(status), None) => Ended::Exited(status.code()),
    (Ok(_), Some(ended)) => ended,
    (Err(error), _) => Ended::Failed(error.to_string()),
  }
}

/// Reads `stderr` to its end on a thread of its own, keeping its start.
fn drain(stderr: impl Read + Send + 'static) -> thread::JoinHandle<String> {
  thread::spawn(move || {
    let mut kept = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut reader = stderr;
    while let Ok(read) = reader.read(&mut buffer) {
      if read == 0 {
        break;
      }
      let room = MAX_STDERR.saturating_sub(kept.len());
      kept.extend_from_slice(&buffer[..read.min(room)]);
    }
    String::from_utf8_lossy(&kept).trim().to_owned()
  })
}

/// Reads the next line of `reader` into `line`, keeping at most [`MAX_LINE`] bytes of it; `false` at
/// the end of the output. Nothing longer is ever held in memory.
fn read_line(reader: &mut impl BufRead, line: &mut Vec<u8>) -> io::Result<bool> {
  line.clear();
  let mut any = false;
  loop {
    let available = reader.fill_buf()?;
    if available.is_empty() {
      return Ok(any);
    }
    any = true;
    let (taken, ended) = match available.iter().position(|byte| *byte == b'\n') {
      Some(end) => (end + 1, true),
      None => (available.len(), false),
    };
    let room = MAX_LINE.saturating_sub(line.len());
    line.extend_from_slice(&available[..taken.min(room)]);
    reader.consume(taken);
    if ended {
      return Ok(true);
    }
  }
}
