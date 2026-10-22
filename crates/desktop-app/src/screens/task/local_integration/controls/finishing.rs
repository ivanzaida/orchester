//! The step beside work found integrated outside Orchester while the record still holds the person's work: recording
//! the merge Git proves as the record's own, after which preserved originals are put back, or otherwise the action
//! that works with the checkout as it is now. Nothing is merged, synced or resolved again.

use lurq::{app::ctx::Ctx, components::Text, node::Element};
use orchester_core::integration::local::{LocalOperation, LocalPhase};

use super::{Action, Inspector, Mode, Props, action_panel, button, mode_button, phase_title};
use crate::{theme::LocalText, ui::button::ButtonKind};

impl Inspector {
  /// What is left to do for a record that holds the person's work once the run's work is found integrated outside
  /// Orchester. Work found whole in the branch is recorded as the record's merge where the record stopped at a step
  /// that left nothing under way (`LocalOperation::can_record_merged_outside`), after which preserved originals are
  /// put back. Otherwise: a cancellation keeps its own actions, a task merge in conflict is aborted, a record still
  /// at the checkout it reviewed is cancelled (its originals put back), and one whose checkout moved since is
  /// reconciled first, which reviews the checkout as it is now.
  pub(super) fn finishing(&self, ctx: &Ctx, props: &Props, phase: LocalPhase) -> Element {
    let operation = props.view.operation.as_ref();
    if props.view.outside_merge_provable() && operation.is_some_and(LocalOperation::can_record_merged_outside) {
      return record_panel(ctx, props, phase);
    }
    if phase == LocalPhase::Canceled {
      return self.available(ctx, props, phase);
    }
    let panel = action_panel(ctx, phase_title(phase), Some("local_integration.finishBody"));
    if phase == LocalPhase::TaskConflicts {
      return panel
        .child(mode_button(
          ctx,
          props,
          ("local-finish-abort", ButtonKind::Primary),
          "local_integration.abort",
          Mode::Abort,
        ))
        .into();
    }
    let cancel = operation
      .filter(|operation| at_reviewed_head(operation, &props.view.checkout.head))
      .and_then(|_| self.cancellation(ctx, props, "local-finish-cancel", ButtonKind::Primary));
    match cancel {
      Some(cancel) => panel.child(cancel),
      None => panel.child(button(
        ctx,
        props,
        "local-reconcile",
        "local_integration.reconcile",
        Some(Action::Reconcile),
        false,
      )),
    }
    .into()
  }
}

/// *Record this merge and finish*, under what it does, naming the run's delivery target that holds the work.
fn record_panel(ctx: &Ctx, props: &Props, phase: LocalPhase) -> Element {
  let into = props
    .view
    .merged_outside()
    .unwrap_or(props.view.checkout.branch.as_str());
  let body = ctx.t_args("local_integration.recordOutsideBody", [("branch", into)]);
  action_panel(ctx, phase_title(phase), None)
    .child(
      Text::new(&body)
        .id("local-actions-body")
        .variant(LocalText::Explanation.role()),
    )
    .child(button(
      ctx,
      props,
      "local-record-outside",
      "local_integration.recordOutside",
      Some(Action::RecordMergedOutside),
      false,
    ))
    .into()
}

/// Whether the checkout's `head` is still the commit `operation` reviewed and, with originals preserved, the one
/// they were taken from: a cancellation checks both, so it fails once a merge moved the checkout.
fn at_reviewed_head(operation: &LocalOperation, head: &str) -> bool {
  operation.reviewed().head == head && operation.preservation().is_none_or(|saved| saved.original.head == head)
}
