//! What the Pipelines page shows when its pipelines couldn't be read (`Pipelines · v2` P9 `H4fLj`): in the body's
//! outline, centered, a `danger` glyph on `danger_muted`, *Couldn't read the pipelines*, that nothing was changed, the
//! reason as the store gave it in mono and *Try again*. Nothing pretends to be empty, the counts stay bones, and
//! *New pipeline* waits for a read that succeeds.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_workspace::messages::Message;

use crate::{
  theme::{ExtraRadius, ExtraSpacing, HubSpacing, HubText, IconSize, PipelinesText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
  },
};

const PREFIX: &str = "pipeline-hub-load-error";
/// The glyph's tile (`Tile` `h7IDWg`).
const TILE: f32 = 36.0;

/// Try again: it reads the pipelines again, and while that is under way the button says so.
pub(super) struct Retry {
  pub retrying: bool,
  pub press: Arc<dyn Fn() + Send + Sync>,
}

/// The body in place of the list and the panel.
pub(super) fn failure(ctx: &Ctx, error: &Message, retry: Retry) -> Element {
  let tile = Row::new()
    .id(format!("{PREFIX}-glyph"))
    .width(Dimension::Px(TILE))
    .height(Dimension::Px(TILE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .rounded(ExtraRadius::Tile.role())
    .background(PaletteColor::DangerMuted)
    .child(icon(Icon::CircleAlert, IconSize::Xl, PaletteColor::Danger));
  let line = |id: &str, key: &str, role: HubText| {
    Text::new(&ctx.t(key))
      .id(format!("{PREFIX}-{id}"))
      .variant(role.role())
      .text_align(TextAlign::Center)
  };
  let again = ActionButton::new(
    format!("{PREFIX}-retry"),
    ButtonKind::Secondary,
    ctx.t("common.tryAgain"),
  )
  .icon(Icon::RotateCw);
  let again = if retry.retrying {
    again.loading()
  } else {
    let press = retry.press;
    again.on_click(move || press())
  };
  Column::new()
    .id(format!("{PREFIX}-body"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(
      Column::new()
        .id(PREFIX)
        .describe("state", "failed")
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .child(tile)
        .child(line("title", "pipeline_hub.states.failed.title", HubText::EmptyTitle))
        .child(line(
          "detail",
          "pipeline_hub.states.failed.detail",
          HubText::EmptyDetail,
        ))
        .child(
          Text::new(&message_text(ctx, error))
            .id(format!("{PREFIX}-reason"))
            .variant(PipelinesText::ReadReason.role())
            .text_align(TextAlign::Center),
        )
        .child(
          Row::new()
            .id(format!("{PREFIX}-action-gap"))
            .height(Dimension::Px(HubSpacing::ActionGap.value())),
        )
        .child(again),
    )
    .into()
}

/// *New pipeline* while the pipelines can't be read: there is nothing to add to yet.
pub(super) fn new_unavailable(ctx: &Ctx) -> Element {
  ActionButton::new("pipeline-hub-new", ButtonKind::Primary, ctx.t("pipeline_hub.new"))
    .icon(Icon::Plus)
    .unavailable_with_icon(ctx.t("pipeline_hub.states.newUnavailable"))
    .into()
}
