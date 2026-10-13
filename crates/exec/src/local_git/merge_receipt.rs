use orchester_core::integration::local::CheckedResult;

use super::{LocalError, LocalGit, LocalGitError};

impl LocalGit {
  /// The checked base is compared again; a unique reflog action distinguishes our write from external containment.
  pub fn merge_result(&self, exact: &CheckedResult, action: &str) -> Result<(), LocalGitError> {
    self.ensure(&exact.base)?;
    if exact.base.head == exact.result.head {
      return Err(LocalError::Stale.into());
    }
    let mut owned = self.clone();
    owned.ref_action = Some(action.to_owned());
    owned.text(&[
      "-c",
      "core.logAllRefUpdates=true",
      "merge",
      "--ff-only",
      "--no-overwrite-ignore",
      &exact.result.head,
    ])?;
    Ok(())
  }
  /// Commit only the reviewed resolved index, with a durable action identifying this attempt.
  pub fn commit_task_resolution(&self, message: &str, action: &str) -> Result<(), LocalGitError> {
    let mut owned = self.clone();
    owned.ref_action = Some(action.to_owned());
    owned.finish_task_resolution(message)
  }

  pub fn has_task_commit_receipt(&self, head: &str, action: &str, message: &str) -> Result<bool, LocalGitError> {
    let entry = self.text(&["reflog", "show", "-1", "--format=%H%x00%gs", "HEAD"])?;
    Ok(entry == format!("{head}\0{action}: {message}"))
  }
  pub fn has_merge_receipt(&self, exact: &CheckedResult, action: &str) -> Result<bool, LocalGitError> {
    let entry = self.text(&["reflog", "show", "-1", "--format=%H%x00%gs", "HEAD"])?;
    let Some((head, message)) = entry.split_once('\0') else {
      return Ok(false);
    };
    Ok(head == exact.result.head && message == format!("{action}: Fast-forward"))
  }
}
