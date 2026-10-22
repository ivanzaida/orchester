use std::path::Path;

use orchester_core::integration::local::{Checkout, HookChange, IndexEntry, LocalError, LocalFile, LocalPath};

use super::{LocalGit, LocalGitError};

/// The scratch index the selected commit is built in.
const SELECTED_INDEX: &str = "selected.index";

impl LocalGit {
  /// Builds HEAD → selected index in a separate index. No excluded staged or working bytes are changed.
  /// Changes nothing: the commit itself verifies every file again before the branch moves.
  pub fn selected_tree(&self, reviewed: &Checkout, paths: &[LocalPath]) -> Result<String, LocalGitError> {
    let tree = self.build_selected(reviewed, paths)?;
    // One check after building covers the whole step: any change before or during it shows here.
    reviewed.ensure_current(&self.verified_snapshot()?)?;
    Ok(tree)
  }

  fn build_selected(&self, reviewed: &Checkout, paths: &[LocalPath]) -> Result<String, LocalGitError> {
    if paths.is_empty() {
      return Err(LocalError::Blank.into());
    }
    let index = self.scratch.join(SELECTED_INDEX);
    if let Some(mut old) = super::owned::OwnedFile::open(&self.scratch, &LocalPath::new(SELECTED_INDEX)?, true)? {
      let expected = super::digest(&old.read()?);
      old.remove(&expected)?;
    }
    self.index_command(&["read-tree", "HEAD"], None, &index)?;
    for path in paths {
      let file = reviewed
        .files
        .iter()
        .find(|file| &file.path == path)
        .ok_or(LocalError::Stale)?;
      if file.only_unstaged() {
        return Err(LocalError::WrongPhase.into());
      }
      if let Some(staged) = &file.staged {
        self.set_index(path, Some(staged), Some(&index))?;
      } else if file.head.is_some() {
        self.set_index(path, None, Some(&index))?;
      } else {
        self.add_untracked(&index, path, file)?;
      }
    }
    let tree = self.index_command(&["write-tree"], None, &index)?;
    super::validate_object(&tree)?;
    Ok(tree)
  }

  /// Adds the reviewed bytes of the untracked `file` to `index` as `git add` would: hashed through the checkout's
  /// attributes for `path`, with its reviewed executable bit.
  fn add_untracked(&self, index: &Path, path: &LocalPath, file: &LocalFile) -> Result<(), LocalGitError> {
    let bytes = self.file(path)?.ok_or(LocalError::Stale)?;
    if Some(super::digest(&bytes)) != file.working {
      return Err(LocalError::Stale.into());
    }
    let object = self.put_path_blob(path, &bytes)?;
    let mode = if cfg!(unix) && file.executable {
      0o100755
    } else {
      0o100644
    };
    self.set_index(path, Some(&IndexEntry { mode, object }), Some(index))
  }

  /// Commits the reviewed `tree` of `paths` with `message`. The repository's hooks run first on a copy; if they
  /// fail or change the tree or the message, nothing moves. The branch then moves from the reviewed HEAD to a
  /// commit of exactly `tree`, and only the committed paths of the checkout's index are updated. The checkout
  /// after it comes back, with where a post-commit hook moved HEAD if it did.
  pub fn commit_selected(
    &self,
    reviewed: &Checkout,
    paths: &[LocalPath],
    tree: &str,
    message: &str,
  ) -> Result<(Checkout, Option<HookChange>), LocalGitError> {
    if message.trim().is_empty() {
      return Err(LocalError::Blank.into());
    }
    self.ensure(reviewed)?;
    if self.build_selected(reviewed, paths)? != tree {
      return Err(LocalError::Stale.into());
    }
    let index = self.scratch.join(SELECTED_INDEX);
    let directory = self.commit_copy(&index)?;
    let hooked = self.run_commit_hooks((&index, directory.path()), tree, message)?;
    self.ensure(reviewed)?;
    let commit = self.text(&["commit-tree", tree, "-p", &reviewed.head, "-F", &hooked.file])?;
    super::validate_object(&commit)?;
    let branch = format!("refs/heads/{}", reviewed.branch);
    let subject = message.lines().next().unwrap_or_default();
    let moved = self.raw(
      &[
        "update-ref",
        "-m",
        &format!("commit: {subject}"),
        &branch,
        &commit,
        &reviewed.head,
      ],
      None,
      None,
    )?;
    if moved.code != Some(0) {
      return Err(LocalError::Stale.into());
    }
    self.post_commit_hook(&index, directory.path())?;
    let current = self.index_committed(reviewed, paths, &commit)?;
    let moved = (current.head != commit).then(|| HookChange::MovedHead {
      commit,
      head: current.head.clone(),
    });
    Ok((current, moved))
  }

  /// After the branch moved to `commit`: the original index is still byte-for-byte intact, so replace only the
  /// committed paths, and confirm every reviewed file kept its working content and its excluded staging.
  fn index_committed(&self, reviewed: &Checkout, paths: &[LocalPath], commit: &str) -> Result<Checkout, LocalGitError> {
    let entries = self.tree_entries(commit)?;
    let original_index = self.index_entries()?;
    for path in paths {
      let original = reviewed
        .files
        .iter()
        .find(|file| &file.path == path)
        .ok_or(LocalError::Stale)?;
      if original_index.get(path) != original.staged.as_ref() || super::file_hash(&self.root, path)? != original.working
      {
        return Err(LocalError::Reconcile.into());
      }
      self.set_index(path, entries.get(path), None)?;
    }
    let current = self.verified_snapshot()?;
    let index = self.index_entries()?;
    for file in &reviewed.files {
      if super::file_hash(&self.root, &file.path)? != file.working
        || (!paths.contains(&file.path) && index.get(&file.path) != file.staged.as_ref())
      {
        return Err(LocalError::Reconcile.into());
      }
    }
    Ok(current)
  }

  pub fn set_index(
    &self,
    path: &LocalPath,
    entry: Option<&IndexEntry>,
    index: Option<&Path>,
  ) -> Result<(), LocalGitError> {
    if let Some(entry) = entry {
      if !matches!(entry.mode, 0o100644 | 0o100755) {
        return Err(LocalError::Unsupported.into());
      }
      super::validate_object(&entry.object)?;
    }
    let data = match entry {
      Some(entry) => format!("{:o} {}\t{}\0", entry.mode, entry.object, path),
      None => format!("0 {}\t{}\0", "0".repeat(self.text(&["rev-parse", "HEAD"])?.len()), path),
    };
    let args = ["update-index", "-z", "--index-info"];
    let Some(index) = index else {
      let output = self.quiet_plumbing(&args, Some(data.as_bytes()), None)?;
      if output.code != Some(0) {
        return Err(LocalGitError::Refused);
      }
      // An entry written without stat data reads as "not uptodate" to Git's own merge and abort, in the app and in
      // the person's terminal alike.
      return self.refresh_index();
    };
    if self.quiet_index(&args, Some(data.as_bytes()), index)?.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    Ok(())
  }

  pub(super) fn index_command(
    &self,
    args: &[&str],
    input: Option<&[u8]>,
    index: &Path,
  ) -> Result<String, LocalGitError> {
    let output = self.quiet_index(args, input, index)?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    String::from_utf8(output.bytes)
      .map(|text| text.trim().to_owned())
      .map_err(|_| LocalGitError::Data)
  }

  /// Clears only reviewed paths after their originals have been durably retained and verified.
  pub fn clear_preserved(&self, reviewed: &Checkout) -> Result<Checkout, LocalGitError> {
    let paths = reviewed.files.iter().map(|file| file.path.clone()).collect::<Vec<_>>();
    self.clear_preserved_selected(reviewed, &paths)
  }

  pub fn clear_preserved_selected(&self, reviewed: &Checkout, paths: &[LocalPath]) -> Result<Checkout, LocalGitError> {
    self.ensure(reviewed)?;
    if paths.is_empty()
      || paths
        .iter()
        .any(|path| !reviewed.files.iter().any(|file| file.path == *path))
    {
      return Err(LocalError::Stale.into());
    }
    for file in reviewed.files.iter().filter(|file| paths.contains(&file.path)) {
      self.ensure_file(reviewed, file)?;
      if let Some(entry) = &file.head {
        self.restore_head(reviewed, file, entry)?;
      } else {
        super::write_regular(&self.root, &file.path, file.working.as_ref(), None)?;
        self.set_index(&file.path, None, None)?;
      }
    }
    let clean = self.verified_snapshot()?;
    let excluded = reviewed
      .files
      .iter()
      .filter(|file| !paths.contains(&file.path))
      .cloned()
      .collect::<Vec<_>>();
    if clean.files != excluded || clean.head != reviewed.head || clean.ignored != reviewed.ignored {
      return Err(LocalError::Reconcile.into());
    }
    Ok(clean)
  }

  /// Refused as stale unless `file` is still as `reviewed` recorded it: the same HEAD, no Git operation under way,
  /// and its content, index entry and executable bit unchanged.
  pub(super) fn ensure_file(&self, reviewed: &Checkout, file: &LocalFile) -> Result<(), LocalGitError> {
    if self.text(&["rev-parse", "HEAD"])? != reviewed.head
      || self.operation()?.is_some()
      || super::file_hash(&self.root, &file.path)? != file.working
      || self.index_entries()?.get(&file.path) != file.staged.as_ref()
      || super::executable(&self.root, &file.path)? != file.executable
    {
      return Err(LocalError::Stale.into());
    }
    Ok(())
  }

  /// Puts HEAD's `entry` of `file` back, in the checkout and the index, over the content `reviewed` recorded.
  pub(super) fn restore_head(&self, reviewed: &Checkout, file: &LocalFile, entry: &IndexEntry) -> Result<(), LocalGitError> {
    let bytes = self.checkout_blob(&reviewed.head, &file.path)?;
    super::write_regular(&self.root, &file.path, file.working.as_ref(), Some(&bytes))?;
    let hash = super::digest(&bytes);
    super::set_executable(&self.root, &file.path, &hash, entry.mode == 0o100755)?;
    self.set_index(&file.path, Some(entry), None)
  }
}
