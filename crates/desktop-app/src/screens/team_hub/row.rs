//! A row of the hub's list (`Item · Builder` `tF2OL`, `Item · pencil-design` `U9RFS`): the kind's glyph, the name over
//! what this computer needs, and at its end a tag and a version. Under All the tag names the kind; under a kind with
//! something to fill in on a computer (profiles, MCP servers, tools) the tag names what an MCP server or a tool is
//! and the version stands after it. The chosen row stands inert on `hover` with a 2 `accent` start (`Item · Builder`
//! `n330IO`); the others open the hub on them.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::readiness::Group;

use super::{
  list::Shown,
  model::{Entry, Kind, Listing, form_key},
  words,
};
use crate::{
  routes::{HubFilter, HubView, ProjectPage, Route},
  theme::{ExtraColor, ExtraSpacing, HubText, IconSize},
  ui::{
    agent_mark::AgentMark,
    icon::icon,
    pressable::{inert, opening},
  },
};

/// A tag's height (`Tag` `uJ2rP`).
const TAG_HEIGHT: f32 = 20.0;

pub(super) fn row(ctx: &Ctx, listing: &Listing, entry: &Entry, shown: &Shown<'_>) -> Element {
  let chosen = shown.selected.is_some_and(|selected| selected.id == entry.id);
  let id = format!("team-hub-row-{}", entry.id);
  let button = if chosen {
    inert(lurq::components::Button::empty())
  } else {
    let route = Route::Project(
      shown.project,
      ProjectPage::hub(HubView::new(shown.filter, Some(entry.id))),
    );
    opening(shown.navigation.to(route))
  };
  let start = if chosen {
    ExtraSpacing::Inset.role()
  } else {
    SpacingSize::Lg
  };
  let button = button
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(entry.height(shown.filter)))
    .padding(Padding::new().left(start).right(SpacingSize::Lg))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .describe("state", if chosen { "chosen" } else { "enabled" })
    .describe("group", group_name(entry.group))
    .describe("kind", entry.kind.name())
    .child(mark(ctx, entry))
    .child(text(ctx, listing, entry, &id))
    .with_children(tag(ctx, entry, shown.filter, &id))
    .with_children(version(entry, shown.filter, &id));
  if chosen {
    button
      .background(ExtraColor::Hover.role())
      .border_left(Border::inside(BorderSize::Lg, PaletteColor::Accent))
      .into()
  } else {
    button
      .hovered_style(lurq::node::Style::new().background(ExtraColor::Hover.role()))
      .into()
  }
}

/// The glyph at a row's start: a profile's CLI's own mark, else the glyph of what the resource is.
fn mark(ctx: &Ctx, entry: &Entry) -> Element {
  match entry.cli {
    Some(cli) => AgentMark::of(cli).render(ctx, IconSize::Lg),
    None => icon(entry.glyph(), IconSize::Lg, glyph_color(entry)).into(),
  }
}

const fn glyph_color(entry: &Entry) -> PaletteColor {
  match (entry.group, entry.kind) {
    (Group::NeedsYouHere, _) => PaletteColor::Warning,
    (Group::OffHere, _) => PaletteColor::TextMuted,
    (Group::Ready, Kind::Instruction | Kind::Skill | Kind::Profile) => PaletteColor::TextSecondary,
    (Group::Ready, Kind::McpServer | Kind::Tool) => PaletteColor::Success,
  }
}

/// The name over its line (`Text` in `U9RFS`).
fn text(ctx: &Ctx, listing: &Listing, entry: &Entry, id: &str) -> Column {
  let said = words::line(ctx, listing, entry);
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(&entry.name)
        .id(format!("{id}-name"))
        .variant(HubText::RowName.role())
        .nowrap(),
    )
    .child(
      Text::new(&said.text)
        .id(format!("{id}-line"))
        .variant(HubText::RowLine.role())
        .color(said.color)
        .nowrap(),
    )
}

/// Under All the kind's tag; under a kind with something to fill in, what the MCP server or the tool is.
fn tag(ctx: &Ctx, entry: &Entry, filter: HubFilter, id: &str) -> Option<Row> {
  let key = if filter == HubFilter::All {
    entry.kind.tag_key()
  } else {
    form_key(entry.form?)
  };
  Some(tag_chip(&format!("{id}-tag"), &ctx.t(key)))
}

/// A tag (`Tag` `uJ2rP`).
pub(super) fn tag_chip(id: &str, words: &str) -> Row {
  Row::new()
    .id(id)
    .height(Dimension::Px(TAG_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Snug.role()))
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Sm)
    .child(
      Text::new(words)
        .id(format!("{id}-label"))
        .variant(HubText::RowTag.role())
        .nowrap(),
    )
}

/// The version a computer uses at a row's end (`Meta` in `U9RFS`), under a kind with something to fill in.
fn version(entry: &Entry, filter: HubFilter, id: &str) -> Option<Text> {
  if filter == HubFilter::All || !entry.kind.fills_in() {
    return None;
  }
  Some(
    Text::new(&entry.number?.to_string())
      .id(format!("{id}-version"))
      .variant(HubText::RowLine.role())
      .color(PaletteColor::TextMuted)
      .nowrap(),
  )
}

const fn group_name(group: Group) -> &'static str {
  match group {
    Group::NeedsYouHere => "needs",
    Group::Ready => "ready",
    Group::OffHere => "off",
  }
}
