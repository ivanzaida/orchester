//! The step beside work found integrated outside Orchester while the record still holds the person's work: recording
//! the merge Git proves as the record's own, after which preserved originals are put back, or, for partial
//! evidence, cancelling, aborting or reconciling the record. Nothing is merged, synced or resolved again.

use lurq::{app::ctx::Ctx, components::Text, node::Element};
use orchester_core::integration::local::LocalPhase;

use super::{Action, Inspector, Mode, Props, action_panel, button, mode_button, phase_title};
use crate::{screens::task::local_integration::words, theme::LocalText, ui::button::ButtonKind};

impl Inspector {
  /// What is left to do for a record that holds the person's work once the run's work is found integrated outside
  /// Orchester: nothing is merged, synced or resolved again. Work found whole in the branch is recorded as the
  /// record's merge, after which preserved originals are put back; otherwise the record is cancelled (its originals
  /// put back), its task merge aborted, its kept originals removed, or its state reconciled first.
  pub(super) fn finishing(&self, ctx: &Ctx, props: &Props, phase: LocalPhase) -> Element {
    if phase == LocalPhase::Canceled {
      return self.available(ctx, props, phase);
    }
    if props.view.external.as_ref().is_some_and(words::merged_outside) {
      let body = ctx.t_args(
        "local_integration.recordOutsideBody",
        [("branch", props.view.checkout.branch.as_str())],
      );
      return action_panel(ctx, phase_title(phase), None)
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
        .into();
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
    match self.cancellation(ctx, props, "local-finish-cancel", ButtonKind::Primary) {
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
