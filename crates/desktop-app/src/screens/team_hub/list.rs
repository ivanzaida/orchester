//! The hub's list (`List` `gETjg` in `t4mtQL`): the resources of the filter in three groups by what this computer
//! needs, each under a title with its count (`Section · Needs you here` `o6TEKq`), a row with its glyph, name, line
//! and, under All, its kind's tag (`Item · Builder` `tF2OL`). The chosen row stands inert on `hover` with a 2
//! `accent` start; the others open the hub on them.

use lurq::{
  app::theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::readiness::Group;

use super::{
  model::{Entry, GROUPS, Kind, Listing},
  words,
};
use crate::{
  navigation::Navigation,
  routes::{HubFilter, HubView, ProjectPage, Route},
  theme::{ExtraColor, ExtraSpacing, HubText, IconSize},
  ui::{
    chip::TagChip,
    icon::icon,
    pressable::{inert, opening},
  },
};

/// The list's width (`List` `gETjg`).
pub(super) const LIST_WIDTH: f32 = 440.0;
/// A group's title row (`Section · Needs you here` `o6TEKq`).
const SECTION_HEIGHT: f32 = 32.0;

/// What the list needs besides the resources.
pub(super) struct Shown<'a> {
  pub project: ProjectId,
  pub filter: HubFilter,
  pub selected: Option<&'a Entry>,
  pub navigation: &'a Navigation,
}

pub(super) fn list(ctx: &lurq::app::ctx::Ctx, listing: &Listing, shown: &Shown<'_>) -> Element {
  let groups = GROUPS.into_iter().filter_map(|group| {
    let entries: Vec<&Entry> = listing.group(shown.filter, group).collect();
    if entries.is_empty() {
      return None;
    }
    Some(
      Column::new()
        .width(Dimension::Pct(100.0))
        .child(section(ctx, group, entries.len()))
        .with_children(entries.into_iter().map(|entry| row(ctx, listing, entry, shown))),
    )
  });
  let groups = Column::new()
    .id("team-hub-list")
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .with_children(groups);
  ScrollVertical::new(groups)
    .id("team-hub-list-scroll")
    .width(Dimension::Px(LIST_WIDTH))
    .height(Dimension::Pct(100.0))
    .into()
}

fn section(ctx: &lurq::app::ctx::Ctx, group: Group, count: usize) -> Row {
  let (name, key, color) = match group {
    Group::NeedsYouHere => ("needs", "team_hub.group.needs", PaletteColor::Warning),
    Group::Ready => ("ready", "team_hub.group.ready", PaletteColor::TextMuted),
    Group::OffHere => ("off", "team_hub.group.off", PaletteColor::TextMuted),
  };
  Row::new()
    .id(format!("team-hub-section-{name}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SECTION_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t(key))
        .id(format!("team-hub-section-{name}-title"))
        .variant(HubText::GroupTitle.role())
        .nowrap(),
    )
    .child(
      Text::new(&count.to_string())
        .id(format!("team-hub-section-{name}-count"))
        .variant(HubText::Hint.role())
        .color(color)
        .nowrap(),
    )
}

fn row(ctx: &lurq::app::ctx::Ctx, listing: &Listing, entry: &Entry, shown: &Shown<'_>) -> Element {
  let chosen = shown.selected.is_some_and(|selected| selected.id == entry.id);
  let said = words::line(ctx, listing, entry);
  let id = format!("team-hub-row-{}", entry.id);
  let glyph_color = match (entry.group, entry.kind) {
    (Group::NeedsYouHere, _) => PaletteColor::Warning,
    (Group::OffHere, _) => PaletteColor::TextMuted,
    (Group::Ready, Kind::Instruction | Kind::Skill | Kind::Profile) => PaletteColor::TextSecondary,
    (Group::Ready, Kind::McpServer | Kind::Tool) => PaletteColor::Success,
  };
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(&entry.name)
        .id(format!("{id}-name"))
        .variant(TypographyStyle::Label)
        .nowrap(),
    )
    .child(
      Text::new(&said.text)
        .id(format!("{id}-line"))
        .variant(HubText::Hint.role())
        .color(said.color)
        .nowrap(),
    );
  let tag =
    (shown.filter == HubFilter::All).then(|| TagChip::new(format!("{id}-tag"), ctx.t(entry.kind.tag_key())).mono());
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
    SpacingSize::Md
  };
  let button = button
    .id(id)
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(entry.height(shown.filter)))
    .padding(Padding::new().left(start).right(SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .describe("state", if chosen { "chosen" } else { "enabled" })
    .describe("group", group_name(entry.group))
    .describe("kind", entry.kind.name())
    .child(icon(entry.kind.glyph(), IconSize::Lg, glyph_color))
    .child(text)
    .with_children(tag);
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

const fn group_name(group: Group) -> &'static str {
  match group {
    Group::NeedsYouHere => "needs",
    Group::Ready => "ready",
    Group::OffHere => "off",
  }
}
