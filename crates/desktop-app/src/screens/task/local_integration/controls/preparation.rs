//! Preparing the checkout (`Prepare checkout` `hULqu`, at 960 `qr0cN`): how many reviewed snapshots are selected,
//! *Commit selected changes* (at 960 *Review selected commit*), *Temporarily preserve selected*, *Discard selected*
//! (no design node; in its neighbours' style, as a danger button), cancelling an operation that holds the checkout,
//! and that nothing changes without the person's reviewed selection.

use lurq::{app::ctx::Ctx, components::Text, node::Element};

use super::{Action, Inspector, Mode, Props, action_panel, button, guard_note, mode_button};
use crate::{theme::LocalText, ui::button::ButtonKind};

impl Inspector {
  pub(super) fn prepared(&self, ctx: &Ctx, props: &Props) -> Vec<Element> {
    let mut actions = vec![
      button(
        ctx,
        props,
        "local-sync",
        "local_integration.sync",
        Some(Action::Sync),
        false,
      )
      .into(),
    ];
    actions.extend(self.cancellation(ctx, props, "local-review-cancel", ButtonKind::Ghost));
    actions
  }

  pub(super) fn preparation(&self, ctx: &Ctx, props: &Props) -> Element {
    let selection = props.selected.get();
    let count = super::super::files::selected_count(&props.view, &selection);
    let (title, body, commit, note) = if props.narrow {
      (
        "local_integration.prepareNarrow",
        ctx.t("local_integration.prepareNarrowBody"),
        "local_integration.commitReview",
        "local_integration.prepareNarrowNote",
      )
    } else {
      let body = if count == 1 {
        ctx.t("local_integration.prepareBody.one")
      } else {
        ctx.t_args(
          "local_integration.prepareBody.other",
          [("count", count.to_string().as_str())],
        )
      };
      (
        "local_integration.prepare",
        body,
        "local_integration.commit",
        "local_integration.prepareNote",
      )
    };
    let mut panel = action_panel(ctx, title, None)
      .child(
        Text::new(&body)
          .id("local-actions-body")
          .variant(LocalText::Explanation.role()),
      )
      .child(button(
        ctx,
        props,
        "local-review-commit",
        commit,
        Some(Action::ReviewCommit(selection.commit_paths())),
        !props.view.commit_selectable(&selection.commit_paths()),
      ))
      .child(mode_button(
        ctx,
        props,
        ("local-review-preserve", ButtonKind::Secondary),
        "local_integration.preserveReview",
        Mode::Preserve,
      ))
      .child(mode_button(
        ctx,
        props,
        ("local-review-discard", ButtonKind::Danger),
        "local_integration.discardReview",
        Mode::Discard,
      ));
    if props.view.checkout.clean() {
      panel = panel.child(button(
        ctx,
        props,
        "local-sync",
        "local_integration.sync",
        Some(Action::Sync),
        false,
      ));
    }
    panel
      .with_children(self.cancellation(ctx, props, "local-review-cancel-partial", ButtonKind::Ghost))
      .child(guard_note(ctx, note))
      .into()
  }

  pub(super) fn cancellation(&self, ctx: &Ctx, props: &Props, id: &str, kind: ButtonKind) -> Option<Element> {
    let operation = props.view.operation.as_ref()?;
    if !operation.can_cancel() || !operation.reserves_checkout() {
      return None;
    }
    let label = if operation.preservation().is_some() {
      "local_integration.cancelPreservation"
    } else {
      "local_integration.release"
    };
    Some(mode_button(ctx, props, (id, kind), label, Mode::Cancel).into())
  }
}
