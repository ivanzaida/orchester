//! The panel beside the list (`Detail` `EXAbv` in `t4mtQL`): the chosen resource's head (what it is and its version,
//! its name), its chips (`Chips` `Zi28O`: *Team · vN* and how it stands here) and what its readiness lacks, with the
//! one action that moves it on pinned at its bottom (`Actions` `x3uhYJ`). The panel of each kind is its own task
//! (ORC-832…834): a profile's is [`super::profile`], an MCP server's [`super::mcp`], a tool's [`super::tool`], an
//! instruction's and a skill's [`super::plain`]; where a resource has no sheet of its kind the panel is this frame, with
//! nothing pinned.

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
  readiness::{Group, ReadinessStanding},
  sharing::team::TeamConfiguration,
};

use super::{
  member,
  model::{Entry, Listing},
  words,
};
use crate::{
  theme::{ExtraSpacing, HubSpacing, HubText, IconSize},
  ui::{
    chip::{BadgeTone, badge},
    icon::{Icon, icon},
  },
};

/// What the panel needs besides the resource.
pub(super) struct Shown<'a> {
  pub team: &'a TeamConfiguration,
}

pub(super) fn panel(ctx: &Ctx, listing: &Listing, entry: &Entry, shown: &Shown<'_>) -> Element {
  let body = body()
    .child(head(ctx, entry, None))
    .child(chips(ctx, listing, entry))
    .with_children(member::lock_line(ctx, shown.team))
    .with_children(readiness(ctx, listing, entry));
  frame(entry, body, None).into()
}

/// The column a panel's sections stand in, 24 from its sides and 20 apart.
pub(in crate::screens) fn body() -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      HubSpacing::Sections.role(),
      HubSpacing::BodyTop.role(),
    ))
    .spacing(HubSpacing::BodyTop.role())
}

/// The panel of `entry`: `body` scrolls over the pinned `bar`.
pub(super) fn frame(entry: &Entry, body: Column, bar: Option<Element>) -> Column {
  let scroll = ScrollVertical::new(body)
    .id("team-hub-panel-scroll")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0));
  Column::new()
    .id("team-hub-panel")
    .describe("resource", entry.id.to_string())
    .describe("kind", entry.kind.name())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .child(scroll)
    .with_children(bar)
}

/// The head: what the resource is and its version over its name, and at its end what `end` puts there (*Edit*).
pub(super) fn head(ctx: &Ctx, entry: &Entry, end: Option<Element>) -> Row {
  head_saying(ctx, entry, None, end)
}

/// [`head`] with `version` in the place of the version's number: what the panel of the kind says after what it is.
pub(super) fn head_saying(ctx: &Ctx, entry: &Entry, version: Option<String>, end: Option<Element>) -> Row {
  head_of(entry, (ctx.t(entry.kind.where_key()).to_string(), version), end)
}

/// [`head_saying`] with `kind` in the place of what the resource is: *Tool · HTTP request*.
pub(super) fn head_of(entry: &Entry, (kind, version): (String, Option<String>), end: Option<Element>) -> Row {
  let version = version.or_else(|| entry.number.map(|number| number.to_string()));
  head_named(&entry.name, (kind, version), end)
}

/// The head of a resource called `name`: `kind` and `version` over it, and at its end what `end` puts there. The form
/// that creates a resource draws it with its name still to come.
pub(in crate::screens) fn head_named(
  name: &str,
  (kind, version): (String, Option<String>),
  end: Option<Element>,
) -> Row {
  let mut where_ = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(
      Text::new(&kind)
        .id("team-hub-panel-kind")
        .variant(HubText::PanelWhere.role())
        .nowrap(),
    );
  if let Some(version) = version {
    where_ = where_.child(
      Text::new(&version)
        .id("team-hub-panel-version")
        .variant(HubText::PanelHint.role())
        .nowrap(),
    );
  }
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Xs)
        .child(where_)
        .child(
          Text::new(name)
            .id("team-hub-panel-title")
            .variant(HubText::PanelTitle.role()),
        ),
    )
    .with_children(end)
}

/// What a head puts at its end: `edit`, then the … menu `more` an admin has, 8 apart.
pub(super) fn with_more(edit: Option<Element>, more: Option<Element>) -> Option<Element> {
  match (edit, more) {
    (Some(edit), Some(more)) => Some(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(edit)
        .child(more)
        .into(),
    ),
    (edit, more) => edit.or(more),
  }
}

pub(super) fn chips(ctx: &Ctx, listing: &Listing, entry: &Entry) -> Row {
  chips_saying(ctx, entry, words::chip(ctx, listing, entry), None)
}

/// [`chips`] whose second chip says `(words, tone)` and, when there is `end`, with it at their end (`Time` `TAV5u`):
/// who uses the resource or where it is added. It ends in an ellipsis when it runs out of room.
pub(super) fn chips_saying(
  ctx: &Ctx,
  entry: &Entry,
  (words, tone): (Arc<str>, BadgeTone),
  end: Option<Arc<str>>,
) -> Row {
  chips_of(ctx, entry, vec![(words, tone)], end)
}

/// [`chips_saying`] with any number of chips after *Team · vN*: an admin's draft is one of them (`Chips` in A13
/// `WMtxm`).
pub(super) fn chips_of(ctx: &Ctx, entry: &Entry, badges: Vec<(Arc<str>, BadgeTone)>, end: Option<Arc<str>>) -> Row {
  let team = match entry.number {
    Some(number) => ctx.t_args("team_hub.chip.team", [("version", number.to_string())]),
    None => ctx.t("team_hub.chip.teamUnnumbered"),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(badge("team-hub-chip-team", BadgeTone::Neutral, &team))
    .with_children(badges.into_iter().enumerate().map(|(index, (words, tone))| {
      let id = if index == 0 {
        "team-hub-chip-here".to_owned()
      } else {
        format!("team-hub-chip-{index}")
      };
      badge(&id, tone, &words)
    }))
    .child(Row::new().flex(1.0))
    .with_children(end.map(|end| {
      Text::new(&end)
        .id("team-hub-chips-end")
        .variant(HubText::PanelHint.role())
        .nowrap()
        .min_width(Dimension::Px(0.0))
        .text_overflow(TextOverflow::Elipsis)
    }))
}

/// *used by Builder, Reviewer*: who uses the resource; nothing when no profile does.
pub(super) fn used_by(ctx: &Ctx, names: &[String]) -> Option<Arc<str>> {
  (!names.is_empty()).then(|| ctx.t_args("team_hub.panel.usedBy", [("names", names.join(", "))]))
}

/// *Ready here?*: the parts this computer lacks, one row each; none for a resource that lacks nothing.
fn readiness(ctx: &Ctx, listing: &Listing, entry: &Entry) -> Option<Column> {
  let Some(ReadinessStanding::Missing(missing)) = &entry.standing else {
    return None;
  };
  if entry.here == Group::OffHere {
    return None;
  }
  let rows = missing.missing().iter().enumerate().map(|(index, part)| {
    Row::new()
      .id(format!("team-hub-missing-{index}"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(HubSpacing::Row.role(), HubSpacing::FieldY.role()))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(icon(Icon::TriangleAlert, IconSize::Base, PaletteColor::Warning))
      .child(
        Text::new(&words::missing(ctx, listing, entry, part))
          .variant(HubText::Value.role())
          .flex(1.0),
      )
  });
  Some(
    Column::new()
      .id("team-hub-readiness")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(Text::new(&ctx.t("team_hub.readiness.title")).variant(HubText::PanelLabel.role()))
          .child(
            Text::new(&ctx.t_args(
              "team_hub.readiness.missing",
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

