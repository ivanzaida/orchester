//! What the index reads from git: the files of a revision (`ls-tree`), the paths two revisions
//! differ in (`diff-tree`), and file contents (`cat-file --batch`, streamed: a repository's text
//! does not have to fit in memory at once). Git runs through `orchester-exec` like every program
//! Orchester starts, with the options of [`git_command`]; plumbing commands are used, so the user's
//! diff configuration does not change what is read.

mod batch;

use std::{collections::HashSet, io, path::Path, time::Duration};

use orchester_core::{
  command::{CommandError, CommandOutput, CommandRunner},
  diff::git::git_command,
  revision::Revision,
};
use orchester_exec::ProcessRunner;
use thiserror::Error;

use self::batch::Batch;

const PROGRAM: &str = "git";
/// Listing or comparing trees answers within seconds even in large repositories; a git that takes
/// longer is stuck.
const TIMEOUT: Duration = Duration::from_secs(5 * 60);
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
  #[error("git cat-file sent nothing for too long and was ended")]
  Stalled,
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
  /// Text larger than the index keeps (1 MiB), with its line count.
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
        batch.busy();
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
