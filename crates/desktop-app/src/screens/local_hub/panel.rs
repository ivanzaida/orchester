//! The parts a local project's panels share (`Detail` in L1, L2): the frame (the body scrolling over the action pinned at
//! its bottom), the chips under a head (how the entry stands, who uses it) and *Ready?*, what the entry's readiness
//! lacks. An MCP server's panel is [`super::mcp`], an archived entry's [`super::archived`], a profile's, tool's,
//! instruction's and skill's [`super::editing`].

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  local_archive::Archivable,
  readiness::{Group, ReadinessStanding},
};

use super::{model::Entry, words};
use crate::{
  theme::{HubSpacing, HubText, IconSize},
  ui::{
    chip::{BadgeTone, badge},
    icon::{Icon, icon},
  },
};

/// The panel of `what`: `body` scrolls over the pinned `bar`.
pub(super) fn frame(what: Archivable, body: Column, bar: Option<Element>) -> Element {
  let scroll = ScrollVertical::new(body)
    .id("local-hub-panel-scroll")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0));
  Column::new()
    .id("local-hub-panel")
    .describe("entry", super::row::token(what))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .child(scroll)
    .with_children(bar)
    .into()
}

/// The chips `badges`, then `end` (*used by Designer, Reviewer*) at their end; it ends in an ellipsis when it runs out
/// of room.
pub(super) fn chips_of(badges: Vec<(Arc<str>, BadgeTone)>, end: Option<Arc<str>>) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(
      badges
        .into_iter()
        .enumerate()
        .map(|(index, (words, tone))| badge(&format!("local-hub-chip-{index}"), tone, &words)),
    )
    .child(Row::new().flex(1.0))
    .with_children(end.map(|end| {
      Text::new(&end)
        .id("local-hub-chips-end")
        .variant(HubText::PanelHint.role())
        .nowrap()
        .min_width(Dimension::Px(0.0))
        .text_overflow(TextOverflow::Elipsis)
    }))
}

/// *Ready?*: the parts that are missing, one row each; none for an entry that lacks nothing.
pub(super) fn readiness(ctx: &Ctx, entry: &Entry) -> Option<Column> {
  let Some(ReadinessStanding::Missing(missing)) = &entry.standing else {
    return None;
  };
  if entry.group == Group::OffHere {
    return None;
  }
  let rows = missing.missing().iter().enumerate().map(|(index, part)| {
    Row::new()
      .id(format!("local-hub-missing-{index}"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(icon(Icon::TriangleAlert, IconSize::Base, PaletteColor::Warning))
      .child(
        Text::new(&words::missing(ctx, entry, part))
          .variant(HubText::Value.role())
          .flex(1.0),
      )
  });
  Some(
    Column::new()
      .id("local-hub-readiness")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(Text::new(&ctx.t("local_hub.readiness.title")).variant(HubText::PanelLabel.role()))
          .child(
            Text::new(&ctx.t_args(
              "local_hub.readiness.missing",
              [("count", missing.missing().len().to_string())],
            ))
            .variant(HubText::PanelHint.role())
            .color(PaletteColor::Warning),
          ),
      )
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .rounded(RadiusSize::Md)
          .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
          .with_children(rows),
      ),
  )
}
