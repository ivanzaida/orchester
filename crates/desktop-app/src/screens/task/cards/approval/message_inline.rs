//! Compact denial editor in the 06b shell (n9u8Jn / hmhef).
use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text, TextInput},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::{DenialNote, DenialNoteError};

use super::{answers::Answering, message};
use crate::{
  screens::task::cards::{decisions::Given, frame::held},
  theme::{CardsText, ExtraSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
  },
};

pub(super) fn editor(ctx: &mut Ctx, answering: &Answering<'_>) -> Vec<Element> {
  let id = answering.id;
  let written = answering.message.get();
  let note = DenialNote::new(&written);
  let open = answering.hold(Given::DenyWithMessage);
  let send = match (&note, open.clone()) {
    (Ok(_), Ok(open)) => Some(answering.send_message(open)),
    _ => None,
  };
  let back = ActionButton::new(
    format!("{id}-message-back"),
    ButtonKind::Secondary,
    ctx.t("cards.approval.message.back"),
  )
  .key(ctx.t("cards.approval.inline.escape"))
  .in_form();
  let back = if matches!(open, Err(true)) {
    back.disabled()
  } else {
    let editing = answering.editing.clone();
    back.on_click(move || editing.set(false))
  };
  let sending = ActionButton::new(
    format!("{id}-message-send"),
    ButtonKind::Primary,
    ctx.t("cards.approval.inline.send"),
  )
  .icon(Icon::ArrowUp)
  .key(ctx.t("cards.approval.inline.enter"))
  .in_form();
  let sending = match (&open, send.clone()) {
    (Err(true), _) => sending.loading(),
    (_, Some(send)) => sending.on_click(send),
    (_, None) => sending.disabled(),
  };
  let bar = Row::new()
    .id(format!("{id}-message-answers"))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t_args(
        "cards.approval.inline.told",
        [("agent", answering.request.agent.as_str())],
      ))
      .id(format!("{id}-message-told"))
      .variant(CardsText::Quiet.role())
      .min_width(Dimension::Px(0.0))
      .flex_shrink(1.0),
    )
    .child(Spacer::new().flex(1.0))
    .child(back)
    .child(sending);
  let block = Column::new()
    .id(format!("{id}-message-block"))
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderFocus))
    .rounded(RadiusSize::Md)
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .child(input(ctx, answering))
    .with_children(too_long(ctx, id, note.as_ref().err()))
    .child(bar)
    .on_key_down(message::keys(answering, send, true));
  let mut parts: Vec<Element> = vec![block.into()];
  if answering.decisions.busy() && answering.sending().is_none() {
    parts.push(held(ctx, id));
  }
  parts
}

fn input(ctx: &Ctx, answering: &Answering<'_>) -> TextInput {
  TextInput::styled(
    answering.message.clone(),
    ctx.theme().typography_style(CardsText::DenialMessage.role()),
  )
  .id(format!("{}-message", answering.id))
  .describe("a11y_name", ctx.t("cards.approval.message.field"))
  .placeholder(&ctx.t("cards.approval.message.field"))
  .placeholder_style(ctx.theme().typography_style(CardsText::Context.role()))
  .caret_color(PaletteColor::TextPrimary)
  .width(Dimension::Pct(100.0))
  .multiline()
  .rows(1, 12)
  .ref_element(answering.field.clone())
}

fn too_long(ctx: &Ctx, id: &str, refused: Option<&DenialNoteError>) -> Option<Element> {
  let Some(DenialNoteError::TooLong { chars }) = refused else {
    return None;
  };
  Some(
    field_error(
      &format!("{id}-message-note"),
      &ctx.t_args(
        "cards.approval.message.tooLong",
        [
          ("max", DenialNote::MAX_CHARS.to_string().as_str()),
          ("count", chars.to_string().as_str()),
        ],
      ),
    )
    .into(),
  )
}
