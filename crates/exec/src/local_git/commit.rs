use std::{fs, path::Path};

use orchester_core::integration::local::{Checkout, IndexEntry, LocalError, LocalPath};

use super::{LocalGit, LocalGitError};

impl LocalGit {
  /// Builds HEAD → selected index in a separate index. No excluded staged or working bytes are changed.
  pub fn selected_tree(&self, reviewed: &Checkout, paths: &[LocalPath]) -> Result<String, LocalGitError> {
    self.ensure(reviewed)?;
    if paths.is_empty() {
      return Err(LocalError::Blank.into());
    }
    let index = self.scratch.join("selected.index");
    if index.exists() {
      fs::remove_file(&index).map_err(|_| LocalGitError::Io)?;
    }
    self.index_command(&["read-tree", "HEAD"], None, &index)?;
    for path in paths {
      let file = reviewed
        .files
        .iter()
        .find(|file| &file.path == path)
        .ok_or(LocalError::Stale)?;
      if file.head == file.staged && file.staged.is_some() {
        return Err(LocalError::WrongPhase.into());
      }
      if let Some(staged) = &file.staged {
        self.set_index(path, Some(staged), Some(&index))?;
      } else if file.head.is_some() {
        self.set_index(path, None, Some(&index))?;
      } else {
        self.add_untracked_in_copy(&index, path)?;
      }
    }
    let tree = self.index_command(&["write-tree"], None, &index)?;
    super::validate_object(&tree)?;
    self.ensure(reviewed)?;
    Ok(tree)
  }

  pub fn commit_selected(
    &self,
    reviewed: &Checkout,
    paths: &[LocalPath],
    tree: &str,
    message: &str,
  ) -> Result<Checkout, LocalGitError> {
    if message.trim().is_empty() {
      return Err(LocalError::Blank.into());
    }
    if self.selected_tree(reviewed, paths)? != tree {
      return Err(LocalError::Stale.into());
    }
    self.ensure(reviewed)?;
    let index = self.scratch.join("selected.index");
    let directory = self.commit_copy(&index)?;
    self.ensure(reviewed)?;
    let output = self.scoped_raw(&["commit", "-m", message], None, Some(&index), Some(directory.path()))?;
    let head = self.text(&["rev-parse", "HEAD"])?;
    if head == reviewed.head {
      return Err(if output.code == Some(0) {
        LocalGitError::Data
      } else {
        LocalGitError::Refused
      });
    }
    let actual = self.text(&["rev-parse", "HEAD^{tree}"])?;
    if actual != tree
      || self.text(&["rev-parse", "HEAD^"])? != reviewed.head
      || self.text(&["symbolic-ref", "--short", "HEAD"])? != reviewed.branch
    {
      return Err(LocalError::Reconcile.into());
    }
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let entries = self.tree_entries(&head)?;
    // The original index is still byte-for-byte intact. Replace only the committed paths.
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
    let current = self.snapshot()?;
    for file in &reviewed.files {
      let working = super::file_hash(&self.root, &file.path)?;
      if working != file.working {
        return Err(LocalError::Reconcile.into());
      }
      if !paths.contains(&file.path) && self.index_entries()?.get(&file.path) != file.staged.as_ref() {
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
    let output = if let Some(index) = index {
      self.quiet_index(&args, Some(data.as_bytes()), index)?
    } else {
      self.quiet_plumbing(&args, Some(data.as_bytes()), None)?
    };
    if output.code != Some(0) {
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
    self.ensure(reviewed)?;
    for file in &reviewed.files {
      if self.text(&["rev-parse", "HEAD"])? != reviewed.head
        || self.operation()?.is_some()
        || super::file_hash(&self.root, &file.path)? != file.working
        || self.index_entries()?.get(&file.path) != file.staged.as_ref()
        || super::executable(&self.root, &file.path)? != file.executable
      {
        return Err(LocalError::Stale.into());
      }
      if file.head.is_some() {
        self.text(&[
          "restore",
          "--source=HEAD",
          "--staged",
          "--worktree",
          "--",
          file.path.as_str(),
        ])?;
      } else {
        self.set_index(&file.path, None, None)?;
        super::write_regular(&self.root, &file.path, file.working.as_ref(), None)?;
      }
    }
    let clean = self.snapshot()?;
    if !clean.clean() || clean.head != reviewed.head || clean.ignored != reviewed.ignored {
      return Err(LocalError::Reconcile.into());
    }
    Ok(clean)
  }
}
