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
    if props
      .view
      .operation
      .as_ref()
      .is_some_and(|operation| operation.preservation().is_some())
    {
      actions.push(
        mode_button(
          ctx,
          props,
          "local-review-cancel",
          "local_integration.cancelPreservation",
          Mode::Cancel,
        )
        .into(),
      );
    }
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
        Some(Action::ReviewCommit(props.selected.get())),
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
    if props
      .view
      .operation
      .as_ref()
      .and_then(|operation| operation.preservation())
      .is_some_and(|saved| saved.retained)
    {
      actions.push(
        mode_button(
          ctx,
          props,
          "local-review-cancel-partial",
          "local_integration.cancelPreservation",
          Mode::Cancel,
        )
        .into(),
      );
    }
    actions
  }
}
