//! The files of a run's worktree, which the desktop reads through the project's runner (protocol 48,
//! `docs/architecture/remote-runners.md`, rule 5): whether the worktree is there, the files it lists, and one file's
//! bytes. The runner's executor side reads them with the code viewer's limits; the desktop shows them as it shows the
//! files of its own checkout. A file's bytes travel raw after the reply's JSON head in a binary frame, as a
//! screencast frame's JPEG does ([`crate::frame`]). A listing travels in pages of at most [`MAX_LIST_PAGE_LEN`], so
//! however many files a worktree holds, no reply outgrows a frame.

use std::{fmt, io};

use serde::{Deserialize, Serialize};

use crate::frame::MAX_FRAME_LEN;

/// The largest file the code viewer shows: a larger one is measured, not read.
pub const MAX_VIEW_BYTES: u64 = 5 * 1024 * 1024;

/// The most bytes the names of one page of a listing take as JSON, well inside a frame with the reply around them.
pub const MAX_LIST_PAGE_LEN: usize = MAX_FRAME_LEN as usize / 4;

/// What the desktop asks of a run's worktree ([`crate::Command::RunFiles`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RunFilesRequest {
  /// Whether the worktree is there. Answered with [`RunFilesReply::Present`] or [`RunFilesReply::Gone`].
  Present,
  /// The files git tracks there and the ones it does not ignore, without tracked files that were deleted, from the
  /// `from`th on (0 for the first page). Answered with [`RunFilesReply::Listed`].
  List { from: u64 },
  /// The file `path`, relative to the worktree with `/` between names. Answered with [`RunFilesReply::Read`].
  Read { path: String },
}

/// The runner's answer to a [`RunFilesRequest`] ([`crate::Reply::RunFiles`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RunFilesReply {
  /// The run's worktree is not there (any more).
  Gone,
  /// The run's worktree is there.
  Present,
  /// One page of a listing: `files`, and `next`, where the next page starts, while the listing goes on.
  Listed {
    files: Vec<String>,
    next: Option<u64>,
  },
  Read(RunFileRead),
  /// What was asked could not be read.
  Failed(RunFileFailure),
}

/// One file of the worktree as read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RunFileRead {
  /// The whole file, at most [`MAX_VIEW_BYTES`].
  Bytes(FileBytes),
  /// A file larger than [`MAX_VIEW_BYTES`], not read.
  TooLarge { size: u64 },
}

/// A file's bytes. They travel after the JSON head of a binary frame, not in it.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBytes {
  #[serde(skip)]
  pub bytes: Vec<u8>,
}

impl fmt::Debug for FileBytes {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("FileBytes").field("len", &self.bytes.len()).finish()
  }
}

/// Why a file or the worktree's list could not be read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RunFileFailure {
  /// The path names nothing in the worktree.
  Missing,
  /// The path names a folder.
  Folder,
  /// It is there but cannot be read now.
  Unreadable { kind: IoKind },
  /// The path, or a link on it, leads out of the worktree.
  Outside,
  /// The path lies in `.git` or `.orchester`.
  Hidden,
  /// The path is not one a file can have.
  Invalid,
  /// git was not found.
  GitMissing,
  /// git failed, with what it said.
  Git { detail: String },
}

/// An [`io::ErrorKind`] as it travels; the kinds a read meets keep their name, any other is [`IoKind::Other`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IoKind {
  NotFound,
  PermissionDenied,
  InvalidInput,
  InvalidData,
  TimedOut,
  Interrupted,
  WouldBlock,
  UnexpectedEof,
  Unsupported,
  OutOfMemory,
  Other,
}

impl From<io::ErrorKind> for IoKind {
  fn from(kind: io::ErrorKind) -> Self {
    match kind {
      io::ErrorKind::NotFound => Self::NotFound,
      io::ErrorKind::PermissionDenied => Self::PermissionDenied,
      io::ErrorKind::InvalidInput => Self::InvalidInput,
      io::ErrorKind::InvalidData => Self::InvalidData,
      io::ErrorKind::TimedOut => Self::TimedOut,
      io::ErrorKind::Interrupted => Self::Interrupted,
      io::ErrorKind::WouldBlock => Self::WouldBlock,
      io::ErrorKind::UnexpectedEof => Self::UnexpectedEof,
      io::ErrorKind::Unsupported => Self::Unsupported,
      io::ErrorKind::OutOfMemory => Self::OutOfMemory,
      _ => Self::Other,
    }
  }
}

impl From<IoKind> for io::ErrorKind {
  fn from(kind: IoKind) -> Self {
    match kind {
      IoKind::NotFound => Self::NotFound,
      IoKind::PermissionDenied => Self::PermissionDenied,
      IoKind::InvalidInput => Self::InvalidInput,
      IoKind::InvalidData => Self::InvalidData,
      IoKind::TimedOut => Self::TimedOut,
      IoKind::Interrupted => Self::Interrupted,
      IoKind::WouldBlock => Self::WouldBlock,
      IoKind::UnexpectedEof => Self::UnexpectedEof,
      IoKind::Unsupported => Self::Unsupported,
      IoKind::OutOfMemory => Self::OutOfMemory,
      IoKind::Other => Self::Other,
    }
  }
}
