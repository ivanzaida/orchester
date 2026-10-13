use orchester_core::integration::local::Checkout;

use super::{LocalError, LocalGit, LocalGitError, LocalPath, digest, read_regular};

impl LocalGit {
  /// Sync starts from a clean task checkout; abort must restore that HEAD's exact index.
  pub fn preview_task_abort(&self, before: &Checkout) -> Result<Checkout, LocalGitError> {
    self.ensure_task_abort(before)?;
    let scope = tempfile::Builder::new()
      .prefix("abort-preview-")
      .tempdir_in(&self.scratch)
      .map_err(|_| LocalGitError::Io)?;
    #[cfg(windows)]
    let _scope_owner = super::owned::ExecutionOwnership::private_copy(scope.path(), &self.scratch)?;
    let index = scope.path().join("index");
    self.index_command(&["read-tree", &before.head], None, &index)?;
    let entries = self.quiet_index(&["ls-files", "--stage", "-z"], None, &index)?;
    if entries.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let ignored = self.preview_ignored(&before.head)?;
    self.ensure_task_abort(before)?;
    let mut result = before.clone();
    result.index = digest(&entries.bytes);
    result.ignored = ignored;
    result.files.clear();
    result.operation = None;
    Ok(result)
  }

  pub fn abort_task_merge(&self, before: &Checkout) -> Result<(), LocalGitError> {
    self.ensure_task_abort(before)?;
    self.text(&["merge", "--abort"])?;
    Ok(())
  }

  fn ensure_task_abort(&self, before: &Checkout) -> Result<(), LocalGitError> {
    if !before
      .operation
      .as_ref()
      .is_some_and(|marker| marker.starts_with("MERGE_HEAD:"))
      || self.snapshot()? != *before
    {
      return Err(LocalError::ForeignOperation.into());
    }
    // Git abort uses ORIG_HEAD and may apply an autostash: neither may introduce foreign work.
    let directory = self.git_dir.as_ref().ok_or(LocalGitError::Data)?;
    if self.text(&["rev-parse", "--verify", "ORIG_HEAD"])? != before.head
      || read_regular(directory, &LocalPath::new("MERGE_AUTOSTASH")?)?.is_some()
    {
      return Err(LocalError::ForeignOperation.into());
    }
    Ok(())
  }
}
