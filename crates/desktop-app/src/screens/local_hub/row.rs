//! A row of the hub's list in a local project (`Item · pencil-design` in L2): the kind's glyph, the name over what the
//! person has to do, and at its end a tag. A local MCP server has no version, so no row of it says one. The chosen row
//! stands inert on `hover` with a 2 `accent` start; the others open the hub on them.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::ResourceKind;
use orchester_workspace::{local_archive::Archivable, readiness::Group};

use super::{
  list::Shown,
  model::{Entry, view_of},
  words::{self, Unsaved},
};
use crate::{
  routes::{HubFilter, ProjectPage, Route},
  screens::team_hub::row::tag_chip,
  theme::{ExtraColor, ExtraSpacing, HubText, IconSize},
  ui::{
    agent_mark::AgentMark,
    icon::{Icon, icon},
    pressable::{inert, opening},
  },
};

/// The element id of an entry's parts.
pub(super) fn token(what: Archivable) -> String {
  match what {
    Archivable::Resource(id) => id.to_string(),
    Archivable::McpServer(id) => format!("mcp-{id}"),
  }
}

pub(super) fn row(ctx: &Ctx, entry: &Entry, shown: &Shown<'_>) -> Element {
  let chosen = shown.selected == Some(entry.what);
  let id = format!("local-hub-row-{}", token(entry.what));
  let button = if chosen {
    inert(lurq::components::Button::empty())
  } else {
    let route = Route::Project(shown.project, ProjectPage::hub(view_of(shown.filter, Some(entry.what))));
    opening(shown.navigation.to(route))
  };
  let start = if chosen {
    ExtraSpacing::Inset.role()
  } else {
    SpacingSize::Lg
  };
  let unsaved = shown
    .unsaved
    .and_then(|(what, state)| (what == entry.what).then_some(state));
  let button = button
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(entry.height()))
    .padding(Padding::new().left(start).right(SpacingSize::Lg))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .describe("state", if chosen { "chosen" } else { "enabled" })
    .describe("group", group_name(entry.group))
    .describe("kind", format!("{:?}", entry.kind))
    .child(mark(ctx, entry, unsaved))
    .child(text(ctx, entry, shown, (&id, unsaved)))
    .with_children(tag(ctx, entry, shown.filter, &id));
  if chosen {
    button
      .background(ExtraColor::Hover.role())
      .border_left(Border::inside(BorderSize::Md, PaletteColor::Accent))
      .into()
  } else {
    button
      .hovered_style(lurq::node::Style::new().background(ExtraColor::Hover.role()))
      .into()
  }
}

/// The glyph at a row's start: a profile's CLI's own mark, else the glyph of what it is, in the color of how it stands.
fn mark(ctx: &Ctx, entry: &Entry, unsaved: Option<Unsaved>) -> Element {
  if entry.archived.is_some() {
    return icon(Icon::Archive, IconSize::Lg, PaletteColor::TextMuted).into();
  }
  if let Some(cli) = entry.cli {
    return AgentMark::of(cli).render(ctx, IconSize::Lg);
  }
  let color = match (unsaved, entry.group) {
    (Some(Unsaved::Refused), _) => PaletteColor::Danger,
    (Some(Unsaved::Edited), _) | (None, Group::NeedsYouHere) => PaletteColor::Warning,
    (None, Group::OffHere) => PaletteColor::TextMuted,
    (None, Group::Ready) => match entry.kind {
      ResourceKind::McpServer | ResourceKind::Tool => PaletteColor::Success,
      _ => PaletteColor::TextSecondary,
    },
  };
  icon(entry.glyph(), IconSize::Lg, color).into()
}

/// The name over its line.
fn text(ctx: &Ctx, entry: &Entry, shown: &Shown<'_>, (id, unsaved): (&str, Option<Unsaved>)) -> Column {
  let said = words::line(ctx, shown.now, entry, unsaved);
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
        .width(Dimension::Pct(100.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
}

/// Under All and Archived the kind's tag; under a kind what an MCP server or a tool is.
fn tag(ctx: &Ctx, entry: &Entry, filter: HubFilter, id: &str) -> Option<Row> {
  let key = if matches!(filter, HubFilter::All | HubFilter::Archived) {
    words::kind_tag_key(entry.kind)?
  } else {
    words::form_key(entry.form?)
  };
  Some(tag_chip(&format!("{id}-tag"), &ctx.t(key)))
}

const fn group_name(group: Group) -> &'static str {
  match group {
    Group::NeedsYouHere => "needs",
    Group::Ready => "ready",
    Group::OffHere => "off",
  }
}
