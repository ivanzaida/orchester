//! The failures that sit inside a page (`02 · Patterns` `N5if9`): a section that can't be read is one row in place of
//! its group's rows, a 14 `danger` circle-alert, what couldn't be read in 13/500 and why in 12 `text_muted`, Try again
//! (ghost) at its end; an item that can't be read keeps its row, its name in 13/500 `text_secondary`, *Couldn't be read
//! · why* in 12 `text_muted` and Open log (ghost) at its end. A change that wasn't saved is said by the header's save
//! status, *A change was not saved · Try again* in 12 `danger` beside the icon, and by the field, which keeps the
//! typed value with a `danger` stroke and a hint under it.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::messages::Message;

use super::{Failure, card::log_button, why};
use crate::{
  theme::{ExtraSpacing, FailureSpacing, FailureText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    pressable::{Press, pressed},
  },
};

pub(super) fn section(ctx: &Ctx, failure: &Failure) -> Element {
  let retry = failure.retry.clone().map(|retry| {
    let button = ActionButton::new(format!("{}-retry", failure.id), ButtonKind::Ghost, ctx.t("ui.failure.tryAgain"))
      .icon(Icon::RefreshCw);
    if failure.retrying {
      ActionButton::new(format!("{}-retry", failure.id), ButtonKind::Ghost, ctx.t("ui.failure.tryingAgain")).loading()
    } else {
      button.on_click(move || retry())
    }
  });
  line(
    &failure.id,
    Some(Icon::CircleAlert),
    (
      Text::new(&failure.what)
        .id(format!("{}-title", failure.id))
        .variant(FailureText::RowTitle.role()),
      why(ctx, &failure.error),
    ),
    retry.map(Into::into),
  )
  .describe("state", "failed")
  .into()
}

pub(super) fn row(ctx: &mut Ctx, failure: &Failure, name: Option<&str>) -> Element {
  let reason = message_text(ctx, &failure.error);
  let name = name.map_or_else(|| ctx.t("ui.failure.rowUnnamed"), Arc::from);
  line(
    &failure.id,
    None,
    (
      Text::new(&name)
        .id(format!("{}-title", failure.id))
        .variant(FailureText::RowTitle.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
      ctx.t_args("ui.failure.row", [("reason", reason)]),
    ),
    log_button(ctx, failure, ButtonKind::Ghost).map(Into::into),
  )
  .describe("state", "unreadable")
  .into()
}

/// A row of a group's box: an optional `danger` icon, a title over a hint, then `end`.
fn line(id: &str, glyph: Option<Icon>, (title, hint): (Text, Arc<str>), end: Option<Element>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(FailureSpacing::Parts.role(), FailureSpacing::DetailY.role()))
    .spacing(FailureSpacing::Gap.role())
    .align_items(Alignment::Center)
    .with_children(glyph.map(|glyph| icon(glyph, IconSize::Md, PaletteColor::Danger)))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(title)
        .child(
          Text::new(&hint)
            .id(format!("{id}-why"))
            .variant(FailureText::StillWorks.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .with_children(end)
}

/// The header's save status after a change was not saved: *A change was not saved · Try again*, where Try again
/// (`<id>-retry`) runs `retry`.
pub fn not_saved(ctx: &Ctx, id: &str, retry: impl Fn() + Send + Sync + 'static) -> Element {
  let again = pressed(Press::run(retry))
    .id(format!("{id}-retry"))
    .child(Text::new(&ctx.t("ui.failure.notSavedRetry")).variant(FailureText::NotSaved.role()));
  Row::new()
    .id(id.to_owned())
    .describe("state", "not-saved")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleAlert, IconSize::Sm, PaletteColor::Danger))
    .child(Text::new(&ctx.t("ui.failure.notSaved")).variant(FailureText::NotSaved.role()))
    .child(again)
    .into()
}

/// The hint under a field whose value was not saved because of `error`; the field shows it with its `danger` stroke
/// (`FieldInput::error`) and keeps the value.
pub fn field_hint(ctx: &Ctx, error: &Message) -> Arc<str> {
  ctx.t_args("ui.failure.fieldNotSaved", [("reason", message_text(ctx, error))])
}
