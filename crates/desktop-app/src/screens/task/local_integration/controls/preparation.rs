use lurq::{app::ctx::Ctx, node::Element};

use super::{Action, Inspector, Mode, Props, button, mode_button};

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
    actions.extend(self.cancellation(ctx, props, "local-review-cancel"));
    actions
  }

  pub(super) fn preparation(&self, ctx: &Ctx, props: &Props) -> Vec<Element> {
    let mut actions: Vec<Element> = vec![
      button(
        ctx,
        props,
        "local-inspect",
        "local_integration.inspect",
        Some(Action::Inspect),
        false,
      )
      .into(),
      button(
        ctx,
        props,
        "local-refresh",
        "local_integration.refresh",
        Some(Action::Refresh),
        false,
      )
      .into(),
      button(
        ctx,
        props,
        "local-review-commit",
        "local_integration.commitReview",
        Some(Action::ReviewCommit(props.selected.get().0)),
        props.selected.get().is_empty()
          || props.selected.get().iter().any(|path| {
            props
              .view
              .checkout
              .files
              .iter()
              .find(|file| file.path == *path)
              .is_none_or(|file| file.head == file.staged && file.head.is_some())
          }),
      )
      .into(),
      mode_button(
        ctx,
        props,
        "local-review-preserve",
        "local_integration.preserveReview",
        Mode::Preserve,
      )
      .into(),
    ];
    if props.view.checkout.clean() {
      actions.push(
        button(
          ctx,
          props,
          "local-sync",
          "local_integration.sync",
          Some(Action::Sync),
          false,
        )
        .into(),
      );
    }
    actions.extend(self.cancellation(ctx, props, "local-review-cancel-partial"));
    actions
  }
  pub(super) fn cancellation(&self, ctx: &Ctx, props: &Props, id: &str) -> Option<Element> {
    let operation = props.view.operation.as_ref()?;
    if !operation.can_cancel() || !operation.reserves_checkout() {
      return None;
    }
    let label = if operation.preservation().is_some() {
      "local_integration.cancelPreservation"
    } else {
      "local_integration.release"
    };
    Some(mode_button(ctx, props, id, label, Mode::Cancel).into())
  }
}
