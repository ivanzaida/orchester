//! Classify retained, non-index files under the predicted tree's ignore rules before mutation.
use orchester_core::{blob::ContentHash, integration::local::LocalPath};

use super::{LocalError, LocalGit, LocalGitError, digest};

impl LocalGit {
  pub(super) fn preview_ignored(&self, tree: &str) -> Result<ContentHash, LocalGitError> {
    let others = self.quiet_plumbing(&["ls-files", "--others", "-z"], None, None)?;
    if others.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let text = std::str::from_utf8(&others.bytes).map_err(|_| LocalGitError::Data)?;
    let paths = text
      .split('\0')
      .filter(|path| !path.is_empty() && !path.starts_with(".orchester/"))
      .map(LocalPath::new)
      .collect::<Result<Vec<_>, _>>()?;
    if paths.is_empty() {
      return Ok(digest(&[]));
    }
    let entries = self.tree_entries(tree)?;
    // Sync uses --no-overwrite-ignore. Refuse collisions before it can start an owned merge.
    for path in &paths {
      for tracked in entries.keys() {
        if super::path_collision::collides(&self._root_owner, path, tracked)? {
          return Err(LocalError::Unsupported.into());
        }
      }
    }
    let scope = tempfile::Builder::new()
      .prefix("ignore-preview-")
      .tempdir_in(&self.scratch)
      .map_err(|_| LocalGitError::Io)?;
    #[cfg(windows)]
    let _scope_owner = super::owned::ExecutionOwnership::private_copy(scope.path(), &self.scratch)?;
    let index = scope.path().join("index");
    self.index_command(&["read-tree", tree], None, &index)?;
    let copy = self.commit_copy(&index)?;
    // Untracked nested rules also affect classification; arbitrary ignored content need not be copied.
    for path in paths
      .iter()
      .filter(|path| path.as_str().rsplit('/').next() == Some(".gitignore"))
    {
      let executable = super::executable(&self.root, path)?;
      super::copy_untracked_file(&self.root, copy.path(), path, executable)?;
    }
    let mut input = Vec::new();
    for path in paths {
      input.extend_from_slice(path.as_str().as_bytes());
      input.push(0);
    }
    // check-ignore takes literal filenames on stdin and rejects the literal-pathspec magic
    // injected by our normal Git environment. Disable that magic only for this read-only command.
    let classified = self.scoped_raw(
      &["--no-literal-pathspecs", "check-ignore", "--no-index", "-z", "--stdin"],
      Some(&input),
      Some(&index),
      Some(copy.path()),
    )?;
    if !matches!(classified.code, Some(0 | 1)) {
      return Err(LocalGitError::Refused);
    }
    // The commit/check and abort contracts require a clean task checkout. Exposed files must be
    // reviewed separately; do not mutate Git and then discover an unsupported untracked outcome.
    if classified.bytes != input {
      return Err(LocalError::Unsupported.into());
    }
    Ok(digest(&classified.bytes))
  }
}
