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
pub(crate) fn evidence_body(kind: orchester_core::integration::local::ResultEvidenceKind) -> &'static str {
  use orchester_core::integration::local::ResultEvidenceKind;
  match kind {
    ResultEvidenceKind::CompleteAncestry => "local_integration.fullAncestry",
    ResultEvidenceKind::PartialAncestry => "local_integration.partialAncestry",
    ResultEvidenceKind::EquivalentPatches => "local_integration.equivalentPatches",
    ResultEvidenceKind::EquivalentTree => "local_integration.equivalentTree",
    ResultEvidenceKind::UnknownSource => "local_integration.unknownSource",
  }
}

/// The Summary's own words for work merged without Orchester: the run's `branch` is in `into`, its delivery target.
pub(crate) fn merged_outside_words(
  ctx: &lurq::app::ctx::Ctx,
  branch: &str,
  into: &str,
) -> (std::sync::Arc<str>, std::sync::Arc<str>) {
  (
    ctx.t("summary.notice.mergedTitle"),
    ctx.t_args("summary.notice.mergedOutsideBody", [("branch", branch), ("into", into)]),
  )
}
