use super::{LocalOperation, LocalPhase};

impl LocalOperation {
  /// Read-only reviews release the checkout, also one paused or stopped while it only reviewed; mutation and
  /// retained recovery keep it reserved.
  pub fn reserves_checkout(&self) -> bool {
    match self.phase {
      LocalPhase::Complete => false,
      LocalPhase::Canceled => self.preservation.as_ref().is_some_and(|saved| saved.retained),
      LocalPhase::Review | LocalPhase::Prepared => self.holds_work(),
      LocalPhase::Paused if matches!(self.paused_from, Some(LocalPhase::Review | LocalPhase::Prepared)) => {
        self.holds_work()
      }
      _ => true,
    }
  }

  /// The phase once the run's merge is recorded. Only originals still retained are put back
  /// ([`LocalPhase::MergedAwaitingRestore`]); a cancellation already put them back, so it stays canceled until its
  /// retained originals are removed, which then completes it. Originals removed already leave nothing to restore.
  pub(super) fn merged_phase(&self) -> LocalPhase {
    match &self.preservation {
      Some(saved) if saved.retained && self.phase == LocalPhase::Canceled => LocalPhase::Canceled,
      Some(saved) if saved.retained => LocalPhase::MergedAwaitingRestore,
      _ => LocalPhase::Complete,
    }
  }

  /// What a review has started and must keep: preserved originals, a merge, a commit, the run's worktree it took
  /// over, a resolution under way or unsaved editor decisions.
  fn holds_work(&self) -> bool {
    self.preservation.is_some()
      || self.merged.is_some()
      || self.commit_tree.is_some()
      || self.owned_task.is_some()
      || self.pending_resolution.is_some()
      || !self.editor_drafts.is_empty()
  }
}
