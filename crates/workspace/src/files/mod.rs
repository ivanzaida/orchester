//! The project's files as the read-only code viewer reads them (`docs/design/code-viewer.md`): a
//! file and the tree at a version (the project's checkout, a run's worktree, a commit or the base
//! branch), the versions a file has, file-name search and search within a file. Only files inside the
//! project's folder or the run's worktree are read, through the file scope agents' file tools use;
//! `.git` and `.orchester` are never shown. Only read-only git commands run. Everything here blocks on
//! the file system and git; the UI calls it off the UI thread.

mod list;
mod open;
mod path;
mod read;
mod source;
mod text;
mod version;
mod versions;

use std::{collections::HashSet, io, path::Path};

use orchester_core::command::CommandRunner;
use thiserror::Error;

use self::source::{Source, git_output, source};
pub use self::{
  list::{EntryKind, FileList, NameMatches, TreeRow},
  open::{OpenError, open_target, runs_when_opened},
  path::{FilePath, PathError, is_hidden},
  read::{FileView, MAX_VIEW_BYTES, read_file},
  text::{FileText, LineRange, LineRangeError, MAX_MATCHES, TextMatch},
  version::{Version, VersionParseError},
  versions::{FileFacts, RunFacts, VersionChoice, Versions, versions},
};
use crate::messages::Message;

/// Why the viewer cannot show a file or a version's files.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum FilesError {
  #[error(transparent)]
  Path(#[from] PathError),
  #[error("the version {0} no longer exists")]
  VersionGone(Version),
  #[error("{0} is not in this version")]
  Missing(FilePath),
  #[error("{0} is a folder")]
  Folder(FilePath),
  #[error("{path} cannot be read: {kind}")]
  Unreadable { path: FilePath, kind: io::ErrorKind },
  #[error("git was not found")]
  GitMissing,
  #[error("git failed: {0}")]
  Git(String),
  #[error("the project's database cannot be read: {0}")]
  Store(String),
}

impl FilesError {
  pub fn message(&self) -> Message {
    match self {
      Self::Path(error) => path_message(error),
      Self::VersionGone(version) => Message::new(match version {
        Version::Checkout => "files.failure.checkoutGone",
        Version::Run(_) => "files.failure.worktreeGone",
        Version::Revision(_) => "files.failure.revisionGone",
        Version::Base => "files.failure.noBranch",
      }),
      Self::Missing(path) => Message::new("files.failure.missing").with("path", path.to_string()),
      Self::Folder(path) => Message::new("files.failure.folder").with("path", path.to_string()),
      Self::Unreadable { path, .. } => Message::new("files.failure.unreadable").with("path", path.to_string()),
      Self::GitMissing => Message::new("files.failure.gitMissing"),
      Self::Git(_) => Message::new("files.failure.git"),
      Self::Store(_) => Message::new("files.failure.store"),
    }
  }
}

fn path_message(error: &PathError) -> Message {
  let (key, path) = match error {
    PathError::Empty => return Message::new("files.refused.empty"),
    PathError::Absolute(path) => ("files.refused.absolute", path),
    PathError::Parent(path) => ("files.refused.parent", path),
    PathError::Invalid(path) => ("files.refused.invalid", path),
    PathError::Hidden(path) => ("files.refused.hidden", path),
    PathError::Outside(path) => ("files.refused.outside", path),
  };
  Message::new(key).with("path", path.clone())
}

/// The files of `version` of the project at `repo_root`. A folder lists what git tracks there and the
/// files it does not ignore, without tracked files that were deleted; a commit lists what it holds.
pub fn list_files(runner: &dyn CommandRunner, repo_root: &Path, version: &Version) -> Result<FileList, FilesError> {
  let listed = |directory: &Path, args: &[&str]| -> Result<Vec<String>, FilesError> {
    let output = git_output(runner, directory, args)?;
    Ok(
      output
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect(),
    )
  };
  match source(runner, repo_root, version)? {
    Source::Folder(folder) => {
      let deleted: HashSet<String> = listed(&folder, &["ls-files", "-z", "--deleted"])?.into_iter().collect();
      let mut files = listed(
        &folder,
        &["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
      )?;
      files.retain(|path| !deleted.contains(path));
      Ok(FileList::new(files))
    }
    Source::Commit { repo, commit, .. } => Ok(FileList::new(listed(
      &repo,
      &["ls-tree", "-r", "-z", "--name-only", &commit],
    )?)),
  }
}
