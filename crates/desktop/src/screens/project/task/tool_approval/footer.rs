//! The dialog's footer (`p2HGY`): *Decide later* with its key at the left, then why an answer did not go
//! through, *Deny* and the primary *Approve once*, which has the focus when the dialog opens. The
//! *Always allow* step's footer has *Back* at the left and its primary at the right (`rn3iP`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension},
};
use orchester_core::run::ApprovalAnswer;

use super::{Pressed, View};
use crate::{
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    button::ActionButton,
    field::wrapping_field_error,
    icon::{Icon, IconSize, icon},
    message_text,
    pressable::{Ring, keyboard_stop},
  },
};

/// Footer height and its buttons' (`p2HGY`).
const FOOTER_HEIGHT: f32 = 76.0;
const BUTTON_HEIGHT: f32 = 36.0;

/// The request step's footer.
pub(super) fn request(ctx: &Ctx, view: &View<'_>) -> Element {
  let deny_label = label(
    ctx,
    view,
    Pressed::Deny,
    "runs.toolApproval.deny",
    "runs.toolApproval.denying",
  );
  let approve_label = label(
    ctx,
    view,
    Pressed::Once,
    "runs.toolApproval.approveOnce",
    "runs.toolApproval.approving",
  );
  bar(ctx, view, decide_later(ctx, view))
    .child(
      ActionButton::new(deny_label)
        .id("tool-approval-deny")
        .icon(Icon::X)
        .in_form()
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(view.answer(Pressed::Deny, ApprovalAnswer::Deny))
        .enabled(!view.busy)
        .render(ctx),
    )
    .child(
      ActionButton::new(approve_label)
        .id("tool-approval-approve-once")
        .icon(Icon::Check)
        .primary()
        .in_form()
        .ref_element(view.primary.clone())
        .height(Dimension::Px(BUTTON_HEIGHT))
        .on_click(view.answer(Pressed::Once, ApprovalAnswer::Once))
        .enabled(!view.busy)
        .render(ctx),
    )
    .into()
}

/// The *Always allow* step's footer: *Back*, and `primary` labelled `primary_label`.
pub(super) fn always(
  ctx: &Ctx,
  view: &View<'_>,
  (primary_label, answer): (Arc<str>, Option<ApprovalAnswer>),
) -> Element {
  let queue = view.props.queue.clone();
  let back = ActionButton::new(ctx.t("runs.toolApproval.always.back"))
    .id("tool-approval-back")
    .icon(Icon::ArrowLeft)
    .in_form()
    .height(Dimension::Px(BUTTON_HEIGHT))
    .on_click(move || queue.always(None))
    .enabled(!view.busy)
    .render(ctx);
  let label = if view.under_way(Pressed::Always) {
    ctx.t("runs.toolApproval.approving")
  } else {
    primary_label
  };
  let primary = ActionButton::new(label)
    .id("tool-approval-always-confirm")
    .icon(Icon::BookmarkPlus)
    .primary()
    .in_form()
    .ref_element(view.primary.clone());
  let primary = match answer {
    Some(answer) => primary.on_click(view.answer(Pressed::Always, answer)),
    None => primary,
  };
  bar(ctx, view, back)
    .child(
      primary
        .height(Dimension::Px(BUTTON_HEIGHT))
        .enabled(!view.busy)
        .render(ctx),
    )
    .into()
}

/// The bar with `left` at its left and why an answer did not go through after it, taking the room
/// left of the buttons and wrapping there, so they keep their size and stay in the dialog.
fn bar(ctx: &Ctx, view: &View<'_>, left: Element) -> Row {
  Row::new()
    .id("tool-approval-actions")
    .width(Dimension::Pct(100.0))
    .min_height(Dimension::Px(FOOTER_HEIGHT))
    .padding_vertical(ExtraSpacing::Step.role())
    .padding_horizontal(ExtraSpacing::Rail.role())
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .child(left)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .with_children(view.error.map(|error| wrapping_field_error(&message_text(ctx, error)))),
    )
}

/// *Decide later* (`iwmQx`): clock, label and its key; closes the dialog and answers nothing.
fn decide_later(ctx: &Ctx, view: &View<'_>) -> Element {
  let queue = view.props.queue.clone();
  let waiting = view.props.waiting.clone();
  let content = Row::new()
    .spacing(ExtraSpacing::Tight.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Clock3, IconSize::Button, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t("runs.toolApproval.decideLater"))
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t("runs.toolApproval.decideLaterKey"))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    );
  let button = Button::empty()
    .button()
    .id("tool-approval-decide-later")
    .height(Dimension::Px(BUTTON_HEIGHT))
    .padding_horizontal(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(content);
  keyboard_stop(button, Ring::Outer)
    .cursor(CursorIcon::Pointer)
    .describe("state", "enabled")
    .on_click(move |_| queue.defer(waiting.iter().cloned()))
    .into()
}

fn label(ctx: &Ctx, view: &View<'_>, pressed: Pressed, idle: &str, busy: &str) -> Arc<str> {
  ctx.t(if view.under_way(pressed) { busy } else { idle })
}
