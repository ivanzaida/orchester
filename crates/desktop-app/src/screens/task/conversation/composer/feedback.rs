//! Under the box, after a send: why it did not go through, with *Try again*, the draft kept; or why the
//! runner did not take a stored message, while it still waits.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::MessageId;
use orchester_workspace::{
  messages::Message,
  tasks::conversation::{Conversation, EntryState, follow::Sent},
};

use super::{
  Press,
  controls::Sending,
  state::{ComposerState, Outcome},
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
/// message, while it still waits and the run's state is the one it was sent in (`mode`).
pub(super) fn line(
  ctx: &mut Ctx,
  state: &ComposerState,
  (conversation, mode): (&Conversation, &str),
  presses: &Sending,
) -> Option<Element> {
  let sent = state.send.state().get();
  if sent.is_pending() {
    return None;
  }
  let (rejected, fulfilled) = (sent.is_rejected(), sent.is_fulfilled());
  if let Some(error) = sent.error.filter(|_| rejected) {
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
  let waits = |id: MessageId| {
    conversation
      .entries
      .iter()
      .any(|entry| entry.id == id && matches!(entry.state, EntryState::Queued(_)))
  };
  let message = match sent.data.filter(|_| fulfilled) {
    Some(Outcome::Message {
      id,
      sent: Sent::Queued(reason),
      mode: sent_in,
      ..
    }) if sent_in == mode && waits(id) => Some(reason),
    _ => None,
  };
  let after_turn = state.after_turn.state().get();
  let fulfilled = after_turn.is_fulfilled();
  let again = match after_turn.data.filter(|_| fulfilled) {
    Some((id, Sent::Queued(reason))) if waits(id) => Some(reason),
    _ => None,
  };
  message.or(again).map(|reason| kept(ctx, &reason))
}

/// A stored message the runner did not take waits, and why.
fn kept(ctx: &Ctx, reason: &Message) -> Element {
  Row::new()
    .id("composer-kept")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Clock, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(&message_text(ctx, reason)).variant(ExtraTypography::Note.role()))
    .into()
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
