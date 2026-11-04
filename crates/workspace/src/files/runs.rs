//! The files of a run's worktree, which the project's runner reads (`docs/architecture/remote-runners.md`, rule 5):
//! the desktop never opens the worktree's folder for them. [`RunFiles`] asks the runner; what it answers is turned
//! into what the code viewer shows of the checkout's files, so a run's file shows as it always did.

use std::{io, sync::Arc};

use orchester_core::id::RunId;
use orchester_ipc::run_files::{RunFileFailure, RunFileRead, RunFilesReply, RunFilesRequest};

use super::{FilePath, FileView, FilesError, PathError, Version, read::view};

/// Asks the runner of the project whose files are read about a run's worktree. Blocks until it answers; the code
/// viewer calls it off the UI thread and off the async workers.
pub trait RunFiles: Send + Sync {
  /// The runner's answer to `request` about the worktree of `run`, or [`FilesError::Runner`] when it gave none.
  fn ask(&self, run: RunId, request: RunFilesRequest) -> Result<RunFilesReply, FilesError>;
}

impl<T: RunFiles + ?Sized> RunFiles for Arc<T> {
  fn ask(&self, run: RunId, request: RunFilesRequest) -> Result<RunFilesReply, FilesError> {
    (**self).ask(run, request)
  }
}

/// Whether the worktree of `run` is there.
pub(super) fn present(runs: &dyn RunFiles, run: RunId) -> Result<bool, FilesError> {
  match runs.ask(run, RunFilesRequest::Present)? {
    RunFilesReply::Present => Ok(true),
    RunFilesReply::Gone => Ok(false),
    other => Err(unexpected(&other)),
  }
}

/// The files the worktree of `run` lists.
pub(super) fn list(runs: &dyn RunFiles, run: RunId) -> Result<Vec<String>, FilesError> {
  match runs.ask(run, RunFilesRequest::List)? {
    RunFilesReply::Listed { files } => Ok(files),
    RunFilesReply::Gone => Err(FilesError::VersionGone(Version::Run(run))),
    RunFilesReply::Failed(failure) => Err(failed(failure, None)),
    other => Err(unexpected(&other)),
  }
}

/// The file `path` of the worktree of `run`, as the viewer shows it.
pub(super) fn read(runs: &dyn RunFiles, run: RunId, path: &FilePath) -> Result<FileView, FilesError> {
  let request = RunFilesRequest::Read {
    path: path.as_str().to_owned(),
  };
  match runs.ask(run, request)? {
    RunFilesReply::Read(RunFileRead::Bytes(file)) => Ok(view(&file.bytes)),
    RunFilesReply::Read(RunFileRead::TooLarge { size }) => Ok(FileView::TooLarge { size }),
    RunFilesReply::Gone => Err(FilesError::VersionGone(Version::Run(run))),
    RunFilesReply::Failed(failure) => Err(failed(failure, Some(path))),
    other => Err(unexpected(&other)),
  }
}

/// `failure` as the viewer tells it, of the file `path` when one was read.
fn failed(failure: RunFileFailure, path: Option<&FilePath>) -> FilesError {
  let of_path = |made: fn(FilePath) -> FilesError| match path {
    Some(path) => made(path.clone()),
    None => FilesError::Runner("a listing answered with a file's failure".to_owned()),
  };
  let shown = || path.map(ToString::to_string).unwrap_or_default();
  match failure {
    RunFileFailure::Missing => of_path(FilesError::Missing),
    RunFileFailure::Folder => of_path(FilesError::Folder),
    RunFileFailure::Unreadable { kind } => match path {
      Some(path) => FilesError::Unreadable {
        path: path.clone(),
        kind: io::ErrorKind::from(kind),
      },
      None => FilesError::Runner(format!("unreadable: {kind:?}")),
    },
    RunFileFailure::Outside => FilesError::Path(PathError::Outside(shown())),
    RunFileFailure::Hidden => FilesError::Path(PathError::Hidden(shown())),
    RunFileFailure::Invalid => FilesError::Path(PathError::Invalid(shown())),
    RunFileFailure::GitMissing => FilesError::GitMissing,
    RunFileFailure::Git { detail } => FilesError::Git(detail),
  }
}

/// A reply that does not answer the request: the runner and the desktop disagree.
fn unexpected(reply: &RunFilesReply) -> FilesError {
  tracing::error!(?reply, "unexpected reply about a run's files");
  FilesError::Runner("an unexpected reply".to_owned())
}
