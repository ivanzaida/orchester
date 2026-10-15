//! Predict a tree's ignored boundary before mutation. Every retained, non-index entry must stay ignored under the
//! tree's rules and must not collide with a path the tree tracks; the prediction lists the entries as status will
//! after the change: a folder that a rule matches and that the tree does not track into as one entry, every other
//! ignored path on its own. Ignored content is never part of a commit or a preservation and is never cleaned.
use std::collections::BTreeSet;

use orchester_core::{blob::ContentHash, integration::local::LocalPath};

use super::{
  LocalError, LocalGit, LocalGitError,
  capture::{Status, ignored_digest},
};

impl LocalGit {
  pub(super) fn preview_ignored(&self, tree: &str) -> Result<ContentHash, LocalGitError> {
    let output = self.raw(super::capture::STATUS, None, None)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let status = Status::parse(&output.bytes)?;
    let retained = status
      .untracked
      .iter()
      .chain(&status.ignored)
      .filter(|entry| !entry.starts_with(".orchester/"))
      .cloned()
      .collect::<BTreeSet<_>>();
    if retained.is_empty() {
      return Ok(ignored_digest(&BTreeSet::new()));
    }
    let entries = self.tree_entries(tree)?;
    // Sync uses --no-overwrite-ignore. Refuse collisions before it can start an owned merge.
    for entry in &retained {
      let path = LocalPath::new(entry.trim_end_matches('/'))?;
      for tracked in entries.keys() {
        if super::path_collision::collides(&self._root_owner, &path, tracked)? {
          return Err(LocalError::Unsupported.into());
        }
      }
    }
    let ignored = self.ignored_under(tree, &entries, &retained)?;
    let tracked_under = |folder: &str| entries.keys().any(|path| path.as_str().starts_with(folder));
    let mut predicted = BTreeSet::new();
    for entry in &retained {
      let folder = ancestors(entry).find(|folder| ignored.contains(*folder) && !tracked_under(folder));
      match folder {
        Some(folder) => predicted.insert(folder.to_owned()),
        // The commit/check and abort contracts require a clean task checkout. Exposed files must be reviewed
        // separately; do not mutate Git and then discover an unsupported untracked outcome.
        None if ignored.contains(entry) => predicted.insert(entry.clone()),
        None => return Err(LocalError::Unsupported.into()),
      };
    }
    Ok(ignored_digest(&predicted))
  }

  /// Which of `retained` and their folders `tree`'s rules ignore, with the untracked `.gitignore` files in place.
  fn ignored_under(
    &self,
    tree: &str,
    entries: &std::collections::BTreeMap<LocalPath, orchester_core::integration::local::IndexEntry>,
    retained: &BTreeSet<String>,
  ) -> Result<BTreeSet<String>, LocalGitError> {
    let scope = tempfile::Builder::new()
      .prefix("ignore-preview-")
      .tempdir_in(&self.scratch)
      .map_err(|_| LocalGitError::Io)?;
    #[cfg(windows)]
    let _scope_owner = super::owned::ExecutionOwnership::private_copy(scope.path(), &self.scratch)?;
    let index = scope.path().join("index");
    self.index_command(&["read-tree", tree], None, &index)?;
    // Only the rules are needed: the tree's ignore files, and untracked ones that apply below their folder.
    let rules = entries
      .iter()
      .filter(|(path, _)| is_rules(path.as_str()))
      .map(|(path, entry)| match entry.mode {
        0o100644 | 0o100755 => Ok(path.clone()),
        _ => Err(LocalError::Unsupported),
      })
      .collect::<Result<Vec<_>, _>>()?;
    let copy = self.partial_copy(&index, &rules)?;
    for entry in retained.iter().filter(|entry| is_rules(entry)) {
      let path = LocalPath::new(entry)?;
      let executable = super::executable(&self.root, &path)?;
      super::copy_untracked_file(&self.root, copy.path(), &path, executable)?;
    }
    let candidates = retained
      .iter()
      .flat_map(|entry| ancestors(entry).chain(std::iter::once(entry.as_str())))
      .collect::<BTreeSet<_>>();
    let mut input = Vec::new();
    for candidate in &candidates {
      input.extend_from_slice(candidate.as_bytes());
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
    let text = std::str::from_utf8(&classified.bytes).map_err(|_| LocalGitError::Data)?;
    Ok(
      text
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect(),
    )
  }
}

fn is_rules(path: &str) -> bool {
  path.rsplit('/').next() == Some(".gitignore")
}

/// The folders above `entry`, outermost first, each ending in `/`.
fn ancestors(entry: &str) -> impl Iterator<Item = &str> {
  let inner = entry.trim_end_matches('/');
  inner.match_indices('/').map(move |(at, _)| &entry[..=at])
}
