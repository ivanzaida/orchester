//! The project's files as the read-only code viewer reads them (`docs/design/code-viewer.md`): a
//! file and the tree at a version (the project's checkout, a run's worktree, a commit or the base
//! branch), the versions a file has, file-name search and search within a file. Only files inside the
//! project's folder or the run's worktree are read, through the file scope agents' file tools use;
//! `.git` and `.orchester` are never shown. Only read-only git commands run. Everything here blocks on
//! the file system and git; the UI calls it off the UI thread.

mod list;
mod path;
mod read;
mod source;
mod text;
mod version;

use std::{collections::HashSet, io, path::Path};

use orchester_core::{command::CommandRunner, id::RunId, run::BranchName};
use thiserror::Error;

pub use self::{
  list::{EntryKind, FileList, TreeRow},
  path::{FilePath, PathError, is_hidden},
  read::{FileView, MAX_VIEW_BYTES, read_file},
  text::{FileText, LineRange, LineRangeError, MAX_MATCHES, TextMatch},
  version::{Version, VersionParseError},
};
use self::{
  read::resolve_in_folder,
  source::{Source, git_output, git_succeeds, source},
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

/// One version a file can be shown at, and whether the file is there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionChoice {
  pub version: Version,
  /// The base branch's name, for [`Version::Base`].
  pub branch: Option<BranchName>,
  /// Whether the file is in this version; `None` without a file.
  pub has_file: Option<bool>,
}

/// The versions that exist for the page: the checkout, the worktree of `run` while it exists, the base
/// branch while the checkout has a branch, and `asked` when it is a commit that exists.
pub fn versions(
  runner: &dyn CommandRunner,
  repo_root: &Path,
  (asked, run): (&Version, Option<RunId>),
  path: Option<&FilePath>,
) -> Result<Vec<VersionChoice>, FilesError> {
  let mut wanted = vec![Version::Checkout];
  wanted.extend(run.map(Version::Run));
  wanted.push(Version::Base);
  if let Version::Revision(_) = asked {
    wanted.push(asked.clone());
  }
  let mut choices = Vec::new();
  for version in wanted {
    let found = match source(runner, repo_root, &version) {
      Ok(found) => found,
      Err(FilesError::VersionGone(_)) => continue,
      Err(error) => return Err(error),
    };
    let branch = match &found {
      Source::Commit { branch, .. } => branch.clone(),
      Source::Folder(_) => None,
    };
    let has_file = path.map(|path| has_file(runner, &found, path)).transpose()?;
    choices.push(VersionChoice {
      version,
      branch,
      has_file,
    });
  }
  Ok(choices)
}

/// Whether `found` holds the file `path`.
fn has_file(runner: &dyn CommandRunner, found: &Source, path: &FilePath) -> Result<bool, FilesError> {
  match found {
    Source::Folder(folder) => Ok(resolve_in_folder(folder, path).is_ok_and(|resolved| resolved.is_file())),
    Source::Commit { repo, commit, .. } => {
      let object = format!("{commit}:{path}");
      git_succeeds(runner, repo, &["cat-file", "-e", &object])
    }
  }
}
