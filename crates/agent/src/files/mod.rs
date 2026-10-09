//! The executors of the native file and search tools (`docs/architecture/native-agent.md` §4.1–§4.5,
//! §5.1, §6.4): `read_file`, `list_files`, `search`, `edit_file` and `write_file`, confined to the
//! step's [`FileScope`]; and of Orchester's file tools for agent CLIs, which read the same way and edit by line range
//! ([`lines`]), search for literal text file by file ([`term`]), also in a folder outside the worktree. A failure is
//! an error result the model can recover from, never a failed loop. Results are plain text within the tool's limits,
//! with a note when they were cut.
//!
//! Reads record a read stamp (length, modified time, content hash) per file; an edit, or a write that
//! replaces a file, needs a stamp that still matches the file, so that the agent never changes a file
//! it has not seen as it is. A compaction drops every stamp: the agent re-reads before it edits.
//!
//! Every method blocks on file I/O and, for `list_files` and `search`, on ripgrep; the runner calls
//! them off its async threads. Reads may run together (§4.10).

mod glob;
mod limits;
mod lines;
mod listing;
mod read;
mod ripgrep;
mod search;
mod stamp;
mod term;
mod walk;
mod write;

use std::{
  io,
  path::{Path, PathBuf},
};

use orchester_core::{
  connection::stream::{CallDetail, FileChange},
  native::{
    edit::EditError,
    item::Item,
    scope::{FileScope, ScopeError},
    session::ToolOutcome,
  },
  orchester_tools::LineEditError,
};
use thiserror::Error;

use self::stamp::Stamps;

/// Why a file tool call failed. The message is the error result the model gets.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub(crate) enum FileError {
  #[error(transparent)]
  Scope(#[from] ScopeError),
  #[error(transparent)]
  Edit(#[from] EditError),
  #[error(transparent)]
  Lines(#[from] LineEditError),
  #[error("{path} cannot be {action}: {kind}")]
  Io {
    path: String,
    action: &'static str,
    kind: io::ErrorKind,
  },
  #[error("{0} does not exist")]
  Missing(String),
  #[error("{0} is a folder; use list_files to see what it holds")]
  Folder(String),
  #[error("{0} is not a folder")]
  NotFolder(String),
  #[error("{path} is {size} bytes, more than read_file reads ({limit} bytes); use search to find what you need in it")]
  TooLarge { path: String, size: u64, limit: u64 },
  #[error("{0} has not been read; read it with read_file before you change it")]
  NotRead(String),
  #[error("{0} changed since you read it; read it again")]
  Stale(String),
  #[error(
    "{path} changed since you read it; nothing was changed. The lines you named are now:\n{now}Read it again before you edit it"
  )]
  ChangedSinceRead { path: String, now: String },
  #[error("{0} does not exist; make a new file with create_file")]
  NoFileToEdit(String),
  #[error("{0} already exists; change it with edit_file after read_file")]
  Exists(String),
  #[error("{0} changed while it was being written; read it again")]
  Moved(String),
  #[error("the glob cannot be used: {0}")]
  Glob(String),
  #[error("{0} is not a regular file")]
  NotRegular(String),
  #[error("{0} appeared while it was being written; read it before you replace it")]
  Appeared(String),
  #[error("ripgrep failed: {0}")]
  Ripgrep(String),
}

/// The file tools of one native invocation.
#[derive(Debug)]
pub struct FileTools {
  scope: FileScope,
  /// The ripgrep program `list_files` and `search` run; without it they walk the folders themselves.
  ripgrep: Option<PathBuf>,
  stamps: Stamps,
}

impl FileTools {
  pub fn new(scope: FileScope, ripgrep: Option<PathBuf>) -> Self {
    Self {
      scope,
      ripgrep,
      stamps: Stamps::default(),
    }
  }

  /// Takes note of items the loop stored: a compaction ends every read, so files are read again
  /// before they are changed (§6.3).
  pub fn stored(&self, items: &[Item]) {
    if items.iter().any(|item| matches!(item, Item::Compaction { .. })) {
      self.stamps.clear();
    }
  }

  /// `path` as the model and Activity see it: relative to the worktree with `/` separators when it
  /// lies there, else in full.
  fn shown(&self, path: &Path) -> String {
    match path.strip_prefix(self.scope.worktree()) {
      Ok(relative) if relative.as_os_str().is_empty() => ".".to_owned(),
      Ok(relative) => relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"),
      Err(_) => path.display().to_string(),
    }
  }

  fn io_error(&self, path: &Path, action: &'static str, error: &io::Error) -> FileError {
    if error.kind() == io::ErrorKind::NotFound {
      return FileError::Missing(self.shown(path));
    }
    FileError::Io {
      path: self.shown(path),
      action,
      kind: error.kind(),
    }
  }
}

/// A successful call's outcome: `detail` says what it read, found or changed.
fn done(content: String, activity: String, changes: Vec<FileChange>, detail: CallDetail) -> ToolOutcome {
  ToolOutcome {
    content: limits::capped(content),
    is_error: false,
    activity,
    exit_code: None,
    changes,
    detail: Some(detail),
  }
}

/// A count as a transcript row records it.
fn count(value: usize) -> u64 {
  u64::try_from(value).unwrap_or(u64::MAX)
}

/// A failed call's outcome.
fn failed(error: &FileError) -> ToolOutcome {
  let message = error.to_string();
  ToolOutcome {
    content: limits::capped(message.clone()),
    is_error: true,
    activity: message,
    exit_code: None,
    changes: Vec::new(),
    detail: None,
  }
}

/// The outcome of a call that ends in `result`.
fn outcome(result: Result<ToolOutcome, FileError>) -> ToolOutcome {
  result.unwrap_or_else(|error| failed(&error))
}
