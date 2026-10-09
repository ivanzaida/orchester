//! Under the box, after a send: why it did not go through, with *Try again*, the draft kept; or why the
//! runner did not take a stored message, which stays queued.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::tasks::conversation::follow::Sent;

use super::{
  Press,
  controls::Sending,
  send::{PlanAction, SendAction},
};
use crate::{
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    pressable::pressable,
  },
};

/// Why the last send did not go through, with *Try again*; or why the runner did not take a stored
/// message, which stays queued.
pub(super) fn line(ctx: &mut Ctx, (sending, plan): (&SendAction, &PlanAction), presses: &Sending) -> Option<Element> {
  let (sent, decided) = (sending.state().get(), plan.state().get());
  if sent.is_pending() || decided.is_pending() {
    return None;
  }
  let failed = sent.error.or(decided.error);
  if let Some(error) = failed {
    let text = ctx.t_args("composer.failed", [("reason", &*message_text(ctx, &error))]);
    let retry = match presses {
      Sending::Ready { primary, .. } => Some(retry(ctx, primary.clone())),
      Sending::Empty | Sending::Busy => None,
    };
    return Some(
      Row::new()
        .id("composer-failed")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(field_error("composer-failed-text", &text).flex_shrink(1.0))
        .with_children(retry)
        .into(),
    );
  }
  let Some(Sent::Queued(reason)) = sent.data else {
    return None;
  };
  let reason = message_text(ctx, &reason);
  Some(
    Row::new()
      .id("composer-kept")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(icon(Icon::Clock, IconSize::Sm, PaletteColor::TextMuted))
      .child(Text::new(&reason).variant(ExtraTypography::Note.role()))
      .into(),
  )
}

/// *Try again*: the failed send again, with the draft as it is.
fn retry(ctx: &Ctx, press: Press) -> Element {
  pressable(move || press())
    .id("composer-retry")
    .child(
      Text::new(&ctx.t("common.tryAgain"))
        .variant(ExtraTypography::NoteStrong.role())
        .color(ExtraColor::AccentText.role())
        .nowrap(),
    )
    .into()
}
