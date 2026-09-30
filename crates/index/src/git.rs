//! What the index reads from git: the files of a revision (`ls-tree`), the paths two revisions
//! differ in (`diff-tree`), and file contents (`cat-file --batch`, streamed: a repository's text
//! does not have to fit in memory at once). Git runs through `orchester-exec` like every program
//! Orchester starts, with the options of [`git_command`]; plumbing commands are used, so the user's
//! diff configuration does not change what is read.

use std::{
  collections::HashSet,
  io::{self, BufRead, BufReader, BufWriter, Read, Write},
  path::Path,
  process::{Child, ChildStderr, ChildStdin},
  thread::{self, JoinHandle},
  time::Duration,
};

use orchester_core::{
  command::{CommandError, CommandOutput, CommandRunner},
  diff::git::git_command,
  revision::Revision,
};
use orchester_exec::ProcessRunner;
use thiserror::Error;

const PROGRAM: &str = "git";
/// Listing or comparing trees answers within seconds even in large repositories; a git that takes
/// longer is stuck.
const TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// As git decides that a file is binary: a NUL byte in its first 8000 bytes.
const BINARY_PROBE: u64 = 8000;
/// The largest file whose text is kept for indexing; a larger one (a generated bundle, a data dump)
/// only has its lines counted as it streams past.
pub(crate) const MAX_TEXT_BYTES: u64 = 1024 * 1024;
/// How much of a large file is held at once while its lines are counted.
const COUNT_CHUNK: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum GitError {
  #[error("cannot run git: {0}")]
  Unavailable(CommandError),
  #[error("`git {command}` failed ({code:?}): {stderr}")]
  Failed {
    command: String,
    code: Option<i32>,
    stderr: String,
  },
  #[error("git printed something that is not {expected}: {detail}")]
  Unrecognized { expected: &'static str, detail: String },
  #[error("cannot read from git: {0}")]
  Stream(io::Error),
}

/// What a tracked path is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryKind {
  File,
  Symlink,
  Submodule,
}

/// A tracked path of a revision and its object.
pub(crate) struct TreeEntry {
  pub(crate) path: String,
  pub(crate) kind: EntryKind,
  pub(crate) object: String,
}

/// A file's content.
pub(crate) enum Blob {
  Text(Vec<u8>),
  /// Text larger than [`MAX_TEXT_BYTES`], with its line count.
  Large {
    lines: usize,
  },
  Binary,
}

/// The repository at `dir`.
pub(crate) struct Repository<'a> {
  dir: &'a Path,
}

impl<'a> Repository<'a> {
  pub(crate) const fn new(dir: &'a Path) -> Self {
    Self { dir }
  }

  fn stdout(&self, args: &[&str]) -> Result<String, GitError> {
    let command = git_command(Path::new(PROGRAM), self.dir, args.iter().copied());
    let output = ProcessRunner::new(TIMEOUT)
      .run(&command)
      .map_err(GitError::Unavailable)?;
    if !output.success() {
      return Err(failure(args, &output));
    }
    Ok(output.stdout)
  }

  /// Every tracked path of `revision`, in git's order.
  pub(crate) fn tree(&self, revision: &Revision) -> Result<Vec<TreeEntry>, GitError> {
    let listing = self.stdout(&["ls-tree", "-r", "-z", "--full-tree", revision.as_str()])?;
    listing
      .split_terminator('\0')
      .map(|line| tree_entry(line).ok_or_else(|| unrecognized("an ls-tree entry", line)))
      .collect()
  }

  /// The paths whose content, kind or presence differs between `from` and `to`.
  pub(crate) fn changed_paths(&self, from: &Revision, to: &Revision) -> Result<HashSet<String>, GitError> {
    let listing = self.stdout(&[
      "diff-tree",
      "-r",
      "-z",
      "--no-renames",
      "--name-only",
      from.as_str(),
      to.as_str(),
    ])?;
    Ok(listing.split_terminator('\0').map(str::to_owned).collect())
  }

  /// Reads the blobs `objects` in order, handing each to `each` with its position.
  pub(crate) fn read_blobs<E: From<GitError>>(
    &self,
    objects: Vec<String>,
    mut each: impl FnMut(usize, Blob) -> Result<(), E>,
  ) -> Result<(), E> {
    let count = objects.len();
    if count == 0 {
      return Ok(());
    }
    let mut batch = Batch::start(self.dir, objects)?;
    let mut read = || -> Result<(), E> {
      for position in 0..count {
        let blob = batch.next()?;
        each(position, blob)?;
      }
      Ok(())
    };
    match read() {
      Ok(()) => Ok(batch.finish()?),
      Err(error) => {
        batch.abandon();
        Err(error)
      }
    }
  }
}

/// A running `git cat-file --batch`: the requests are written by a thread of their own while the
/// answers are read, so neither pipe fills up and blocks git.
struct Batch {
  child: Child,
  output: BufReader<std::process::ChildStdout>,
  requests: JoinHandle<io::Result<()>>,
  errors: JoinHandle<io::Result<String>>,
  expected: std::vec::IntoIter<String>,
}

impl Batch {
  fn start(dir: &Path, objects: Vec<String>) -> Result<Self, GitError> {
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
    Ok(Self {
      child,
      output: BufReader::new(output),
      requests,
      errors: read_errors(errors),
      expected: objects.into_iter(),
    })
  }

  fn next(&mut self) -> Result<Blob, GitError> {
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
  fn finish(mut self) -> Result<(), GitError> {
    drop(self.output);
    let status = self.child.wait().map_err(GitError::Stream)?;
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
  fn abandon(mut self) {
    if let Err(error) = self.child.kill() {
      tracing::warn!(%error, "cannot end git cat-file");
    }
    if let Err(error) = self.child.wait() {
      tracing::warn!(%error, "cannot reap git cat-file");
    }
  }
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

/// `<mode> <type> <object>\t<path>`.
fn tree_entry(line: &str) -> Option<TreeEntry> {
  let (meta, path) = line.split_once('\t')?;
  let kind = match meta.split(' ').collect::<Vec<_>>()[..] {
    ["120000", "blob", object] => (EntryKind::Symlink, object),
    ["160000", "commit", object] => (EntryKind::Submodule, object),
    [_, "blob", object] => (EntryKind::File, object),
    _ => return None,
  };
  Some(TreeEntry {
    path: path.to_owned(),
    kind: kind.0,
    object: kind.1.to_owned(),
  })
}

fn failure(args: &[&str], output: &CommandOutput) -> GitError {
  GitError::Failed {
    command: args.join(" "),
    code: output.code,
    stderr: output.stderr.trim().to_owned(),
  }
}

fn unrecognized(expected: &'static str, text: &str) -> GitError {
  GitError::Unrecognized {
    expected,
    detail: text.chars().take(200).collect(),
  }
}
