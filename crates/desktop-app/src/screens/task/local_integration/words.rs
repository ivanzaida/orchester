use orchester_core::integration::local::LocalPhase;

pub(super) fn phase_title(phase: LocalPhase) -> &'static str {
  match phase {
    LocalPhase::Review => "local_integration.review",
    LocalPhase::Committing => "local_integration.commitTitle",
    LocalPhase::Preserving => "local_integration.preserveTitle",
    LocalPhase::Prepared | LocalPhase::Syncing | LocalPhase::Checking => "local_integration.syncTitle",
    LocalPhase::TaskConflicts => "local_integration.taskConflicts",
    LocalPhase::AwaitingApproval => "local_integration.checked",
    LocalPhase::Merging => "local_integration.merge",
    LocalPhase::MergedAwaitingRestore | LocalPhase::Restoring | LocalPhase::RestoreConflicts => {
      "local_integration.restoreTitle"
    }
    LocalPhase::RestorationVerified | LocalPhase::RemovingPreservation => "local_integration.restored",
    LocalPhase::Complete => "local_integration.complete",
    LocalPhase::Paused => "local_integration.paused",
    LocalPhase::Canceling => "local_integration.cancelTitle",
    LocalPhase::Canceled => "local_integration.canceled",
    LocalPhase::NeedsReconciliation => "local_integration.recoveryTitle",
  }
}
pub(super) fn phase_body(phase: LocalPhase) -> &'static str {
  match phase {
    LocalPhase::Review => "local_integration.reviewBody",
    LocalPhase::TaskConflicts => "local_integration.taskConflictBody",
    LocalPhase::AwaitingApproval => "local_integration.checkedBody",
    LocalPhase::MergedAwaitingRestore | LocalPhase::Restoring | LocalPhase::RestoreConflicts => {
      "local_integration.restoreBody"
    }
    LocalPhase::Complete | LocalPhase::RestorationVerified => "local_integration.completeBody",
    LocalPhase::Canceled | LocalPhase::Canceling => "local_integration.cancelBody",
    _ => "local_integration.recoveryBody",
  }
}
