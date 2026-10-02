//! A running `git cat-file --batch`: the requests are written by a thread of their own while the
//! answers are read, so neither pipe fills up and blocks git. A watchdog ends a git that sends
//! nothing for [`STALL`]: reading blocks, so a stuck git would hold the build forever.

use std::{
  io::{self, BufRead, BufReader, BufWriter, Read, Write},
  path::Path,
  process::{Child, ChildStderr, ChildStdin, ChildStdout},
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
  },
  thread::{self, JoinHandle},
  time::{Duration, Instant},
};

use orchester_core::{command::CommandError, diff::git::git_command};
use parking_lot::Mutex;

use super::{Blob, GitError, PROGRAM, unrecognized};

/// As git decides that a file is binary: a NUL byte in its first 8000 bytes.
const BINARY_PROBE: u64 = 8000;
/// The largest file whose text is kept for indexing; a larger one (a generated bundle, a data dump)
/// only has its lines counted as it streams past.
const MAX_TEXT_BYTES: u64 = 1024 * 1024;
/// How much of a large file is held at once while its lines are counted.
const COUNT_CHUNK: usize = 64 * 1024;
/// How long git may send nothing before it is taken for stuck and ended.
const STALL: Duration = Duration::from_secs(120);
/// How often the watchdog looks.
const WATCH_INTERVAL: Duration = Duration::from_secs(1);
/// The watchdog's "last read" while the reader handles a blob instead of waiting for git.
const BUSY: u64 = u64::MAX;

pub(super) struct Batch {
  child: Arc<Mutex<Child>>,
  output: BufReader<ChildStdout>,
  requests: JoinHandle<io::Result<()>>,
  errors: JoinHandle<io::Result<String>>,
  expected: std::vec::IntoIter<String>,
  watchdog: Watchdog,
}

impl Batch {
  pub(super) fn start(dir: &Path, objects: Vec<String>) -> Result<Self, GitError> {
    let command = git_command(Path::new(PROGRAM), dir, ["cat-file", "--batch"]).with_interactive_stdin();
    let mut child = orchester_exec::prepare(&command)
      .map_err(GitError::Unavailable)?
      .spawn()
      .map_err(|error| {
        GitError::Unavailable(match error.kind() {
          io::ErrorKind::NotFound => CommandError::NotFound,
          _ => CommandError::Failed(error.to_string()),
        })
      })?;
    let piped = "orchester_exec::prepare pipes every stream of an interactive command";
    let input = child.stdin.take().expect(piped);
    let output = child.stdout.take().expect(piped);
    let errors = child.stderr.take().expect(piped);
    let requests = write_requests(input, objects.clone());
    let child = Arc::new(Mutex::new(child));
    Ok(Self {
      watchdog: Watchdog::start(Arc::clone(&child)),
      child,
      output: BufReader::new(output),
      requests,
      errors: read_errors(errors),
      expected: objects.into_iter(),
    })
  }

  /// The caller works on the last blob and reads no more for now: the watchdog waits.
  pub(super) fn busy(&self) {
    self.watchdog.last.store(BUSY, Ordering::Relaxed);
  }

  pub(super) fn next(&mut self) -> Result<Blob, GitError> {
    self.watchdog.progressed();
    let blob = self.read_one();
    match blob {
      Err(_) if self.watchdog.fired() => Err(GitError::Stalled),
      other => {
        self.watchdog.progressed();
        other
      }
    }
  }

  fn read_one(&mut self) -> Result<Blob, GitError> {
    let expected = self
      .expected
      .next()
      .expect("the reader asks for no more blobs than it requested");
    let mut header = String::new();
    self.output.read_line(&mut header).map_err(GitError::Stream)?;
    let size = match header.trim_end().split(' ').collect::<Vec<_>>()[..] {
      [object, "blob", size] if object == expected => size.parse::<u64>().ok(),
      _ => None,
    }
    .ok_or_else(|| unrecognized("a cat-file blob header", &header))?;
    let blob = read_content(&mut self.output, size).map_err(GitError::Stream)?;
    let mut end = [0_u8; 1];
    self.output.read_exact(&mut end).map_err(GitError::Stream)?;
    if end != *b"\n" {
      return Err(unrecognized("the end of a blob", &header));
    }
    Ok(blob)
  }

  /// Waits for git to end after the last answer.
  pub(super) fn finish(self) -> Result<(), GitError> {
    drop(self.output);
    self.watchdog.stop();
    let status = self.child.lock().wait().map_err(GitError::Stream)?;
    let written = self.requests.join().expect("the request writer does not panic");
    let stderr = self
      .errors
      .join()
      .expect("the error reader does not panic")
      .map_err(GitError::Stream)?;
    if !status.success() {
      return Err(GitError::Failed {
        command: "cat-file --batch".into(),
        code: status.code(),
        stderr: stderr.trim().to_owned(),
      });
    }
    written.map_err(GitError::Stream)
  }

  /// Ends git after a failure; the failure is what the caller reports.
  pub(super) fn abandon(self) {
    self.watchdog.stop();
    let mut child = self.child.lock();
    if let Err(error) = child.kill() {
      tracing::warn!(%error, "cannot end git cat-file");
    }
    if let Err(error) = child.wait() {
      tracing::warn!(%error, "cannot reap git cat-file");
    }
  }
}

/// Ends git once it has sent nothing for [`STALL`].
struct Watchdog {
  started: Instant,
  /// Milliseconds after `started` of the last blob read.
  last: Arc<AtomicU64>,
  fired: Arc<AtomicBool>,
  stop: mpsc::Sender<()>,
}

impl Watchdog {
  fn start(child: Arc<Mutex<Child>>) -> Self {
    let started = Instant::now();
    let last = Arc::new(AtomicU64::new(0));
    let fired = Arc::new(AtomicBool::new(false));
    let (stop, stopped) = mpsc::channel::<()>();
    let (watched, flag) = (Arc::clone(&last), Arc::clone(&fired));
    thread::spawn(move || {
      while let Err(mpsc::RecvTimeoutError::Timeout) = stopped.recv_timeout(WATCH_INTERVAL) {
        let last = watched.load(Ordering::Relaxed);
        if last != BUSY && millis(started.elapsed()).saturating_sub(last) > millis(STALL) {
          flag.store(true, Ordering::Relaxed);
          tracing::warn!("git cat-file sent nothing for {} s; ending it", STALL.as_secs());
          if let Err(error) = child.lock().kill() {
            tracing::warn!(%error, "cannot end git cat-file");
          }
          return;
        }
      }
    });
    Self {
      started,
      last,
      fired,
      stop,
    }
  }

  fn progressed(&self) {
    self.last.store(millis(self.started.elapsed()), Ordering::Relaxed);
  }

  fn fired(&self) -> bool {
    self.fired.load(Ordering::Relaxed)
  }

  fn stop(&self) {
    // The watchdog is gone once it has fired; there is nothing left to stop then.
    if self.stop.send(()).is_err() {
      tracing::debug!("the git watchdog has already ended");
    }
  }
}

fn millis(duration: Duration) -> u64 {
  u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn write_requests(input: ChildStdin, objects: Vec<String>) -> JoinHandle<io::Result<()>> {
  thread::spawn(move || {
    // Dropping the writer closes the pipe: git ends after the last answer.
    let mut writer = BufWriter::new(input);
    for object in objects {
      writeln!(writer, "{object}")?;
    }
    writer.flush()
  })
}

fn read_errors(mut errors: ChildStderr) -> JoinHandle<io::Result<String>> {
  thread::spawn(move || {
    let mut text = String::new();
    errors.read_to_string(&mut text)?;
    Ok(text)
  })
}

/// The `size` bytes of a blob; a binary one is skipped after its first bytes, a large one after
/// its lines are counted.
fn read_content(reader: &mut impl BufRead, size: u64) -> io::Result<Blob> {
  let probe = size.min(BINARY_PROBE);
  let mut content = Vec::new();
  reader.by_ref().take(probe).read_to_end(&mut content)?;
  if content.contains(&0) {
    let skipped = io::copy(&mut reader.by_ref().take(size - probe), &mut io::sink())?;
    return complete(probe + skipped, size).map(|()| Blob::Binary);
  }
  if size > MAX_TEXT_BYTES {
    return count_lines(reader, &content, size);
  }
  reader.by_ref().take(size - probe).read_to_end(&mut content)?;
  complete(content.len() as u64, size)?;
  Ok(Blob::Text(content))
}

/// The line count of a large blob whose first bytes are `head`, read to its end a chunk at a time.
fn count_lines(reader: &mut impl BufRead, head: &[u8], size: u64) -> io::Result<Blob> {
  let mut breaks = head.iter().filter(|byte| **byte == b'\n').count();
  let mut last = head.last().copied();
  let mut read = head.len() as u64;
  let mut rest = reader.by_ref().take(size - read);
  let mut chunk = vec![0; COUNT_CHUNK];
  loop {
    let count = rest.read(&mut chunk)?;
    if count == 0 {
      break;
    }
    breaks += chunk[..count].iter().filter(|byte| **byte == b'\n').count();
    last = Some(chunk[count - 1]);
    read += count as u64;
  }
  complete(read, size)?;
  Ok(Blob::Large {
    lines: breaks + usize::from(last.is_some_and(|last| last != b'\n')),
  })
}

fn complete(read: u64, size: u64) -> io::Result<()> {
  if read == size {
    Ok(())
  } else {
    Err(io::Error::new(
      io::ErrorKind::UnexpectedEof,
      format!("git sent {read} of {size} bytes"),
    ))
  }
}
