//! Reading one file of a version for the code viewer. A folder's file is checked with the file scope
//! agents' file tools use (`FileScope::repository_readable`): a link or junction that leads out of the
//! folder, and `.git` and `.orchester`, are refused. A commit's file is read with `git ls-tree` and
//! `git cat-file`; what it stores is what git holds, so no link is followed. Files above
//! [`MAX_VIEW_BYTES`] are measured but not read, and a NUL byte in the first 8 KiB makes a file binary,
//! as for agents' `read_file`.

use std::{fs, io, path::Path, sync::Arc};

use orchester_core::{
  command::CommandRunner,
  native::scope::{FileScope, ScopeError, ScopeRoots},
  permission::RepositoryAccess,
};

use super::{
  Digest, FilePath, FilesError, PathError, Version,
  source::{LITERAL_PATHS, Source, git_output, source},
  text::FileText,
};

/// The largest file the viewer shows.
pub const MAX_VIEW_BYTES: u64 = 5 * 1024 * 1024;
/// Bytes looked at for a NUL to tell a binary file.
const BINARY_PROBE: usize = 8 * 1024;
/// The mode git gives a symbolic link.
const LINK_MODE: &str = "120000";

/// What the viewer shows of a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileView {
  Text(Arc<FileText>),
  Binary { size: u64 },
  TooLarge { size: u64 },
}

impl FileView {
  /// A digest of what the view shows, to tell a read again that found the same: a binary or too large file shows
  /// its size alone.
  pub fn digest(&self) -> Digest {
    match self {
      Self::Text(text) => text.digest(),
      Self::Binary { size } => Digest::binary(*size),
      Self::TooLarge { size } => Digest::too_large(*size),
    }
  }
}

/// The file `path` in `version` of the project at `repo_root`. Blocks on the file system and git.
pub fn read_file(
  runner: &dyn CommandRunner,
  repo_root: &Path,
  version: &Version,
  path: &FilePath,
) -> Result<FileView, FilesError> {
  match source(runner, repo_root, version)? {
    Source::Folder(folder) => read_in_folder(&folder, path),
    Source::Commit { repo, commit, .. } => read_in_commit(runner, &repo, &commit, path),
  }
}

/// Where `path` lies in `folder` with its links resolved, or why it may not be read.
pub(super) fn resolve_in_folder(folder: &Path, path: &FilePath) -> Result<std::path::PathBuf, FilesError> {
  let scope = FileScope::new(&ScopeRoots {
    worktree: folder,
    access: RepositoryAccess::ReadOnly,
    code_index: None,
    skills: None,
    writable_folders: &[],
    run_storage: None,
  })
  .map_err(|error| scope_error(path, &error))?;
  scope
    .repository_readable(path.as_str())
    .map_err(|error| scope_error(path, &error))
}

fn scope_error(path: &FilePath, error: &ScopeError) -> FilesError {
  let shown = path.to_string();
  match error {
    ScopeError::NotReadable(_) | ScopeError::NetworkPath(_) => FilesError::Path(PathError::Outside(shown)),
    ScopeError::Protected(_) => FilesError::Path(PathError::Hidden(shown)),
    ScopeError::Unresolved { kind, .. } => FilesError::Unreadable {
      path: path.clone(),
      kind: *kind,
    },
    ScopeError::Nul
    | ScopeError::DevicePath(_)
    | ScopeError::DeviceName(_)
    | ScopeError::AlternateStream(_)
    | ScopeError::DriveRelative(_)
    | ScopeError::NotWritable(_) => FilesError::Path(PathError::Invalid(shown)),
  }
}

fn read_in_folder(folder: &Path, path: &FilePath) -> Result<FileView, FilesError> {
  let resolved = resolve_in_folder(folder, path)?;
  let unreadable = |error: io::Error| match error.kind() {
    io::ErrorKind::NotFound => FilesError::Missing(path.clone()),
    kind => FilesError::Unreadable {
      path: path.clone(),
      kind,
    },
  };
  let metadata = fs::metadata(&resolved).map_err(unreadable)?;
  if metadata.is_dir() {
    return Err(FilesError::Folder(path.clone()));
  }
  if !metadata.is_file() {
    return Err(FilesError::Unreadable {
      path: path.clone(),
      kind: io::ErrorKind::Unsupported,
    });
  }
  if metadata.len() > MAX_VIEW_BYTES {
    return Ok(FileView::TooLarge { size: metadata.len() });
  }
  let bytes = fs::read(&resolved).map_err(unreadable)?;
  Ok(view(&bytes))
}

fn read_in_commit(
  runner: &dyn CommandRunner,
  repo: &Path,
  commit: &str,
  path: &FilePath,
) -> Result<FileView, FilesError> {
  let listed = git_output(
    runner,
    repo,
    &[LITERAL_PATHS, "ls-tree", "-l", "-z", commit, "--", path.as_str()],
  )?;
  let entry = listed.split('\0').next().unwrap_or_default();
  if entry.is_empty() {
    return Err(FilesError::Missing(path.clone()));
  }
  let (mode, kind, object, size) = ls_tree_entry(entry).ok_or_else(|| FilesError::Git(entry.to_owned()))?;
  if kind != "blob" {
    return Err(FilesError::Folder(path.clone()));
  }
  let size: u64 = size.parse().map_err(|_| FilesError::Git(entry.to_owned()))?;
  if size > MAX_VIEW_BYTES {
    return Ok(FileView::TooLarge { size });
  }
  let text = git_output(runner, repo, &["cat-file", "blob", object])?;
  // A link at a commit holds the path it names, shown as its text and never followed. Git's output
  // reaches here as text, in which a NUL byte stays a NUL character.
  if mode != LINK_MODE && text.bytes().take(BINARY_PROBE).any(|byte| byte == 0) {
    return Ok(FileView::Binary { size });
  }
  Ok(FileView::Text(Arc::new(FileText::parse(text.as_bytes()))))
}

/// `<mode> <type> <object> <size>` of a `git ls-tree -l` entry, before its tab and path.
fn ls_tree_entry(entry: &str) -> Option<(&str, &str, &str, &str)> {
  let (meta, _) = entry.split_once('\t')?;
  let mut fields = meta.split_whitespace();
  let entry = (fields.next()?, fields.next()?, fields.next()?, fields.next()?);
  entry.2.bytes().all(|byte| byte.is_ascii_hexdigit()).then_some(entry)
}

/// What `bytes`, read whole, show as.
fn view(bytes: &[u8]) -> FileView {
  if bytes.iter().take(BINARY_PROBE).any(|byte| *byte == 0) {
    return FileView::Binary {
      size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    };
  }
  FileView::Text(Arc::new(FileText::parse(bytes)))
}
