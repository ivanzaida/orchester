//! The hub's list (`List` `gETjg` in `t4mtQL`): the resources of the filter in three groups by what this computer
//! needs, each under a title with its count (`Section · Needs you here` `o6TEKq`), a row with its glyph, name, line
//! and at its end its tag and version (`Item · Builder` `tF2OL`, [`super::row`]); the instructions' and skills' lists
//! say under their group why every one is ready (`Why` `e1C14P`).

use lurq::{
  app::theme::{PaletteColor, SpacingSize},
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::readiness::Group;

use super::{
  create,
  model::{Entry, GROUPS, Kind, Listing},
  row::row,
};
use crate::{
  navigation::Navigation,
  routes::HubFilter,
  theme::{ExtraSpacing, HubSpacing, HubText},
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
  /// The kind whose form is open in the panel: its *New* row stands over the groups, chosen.
  pub adding: Option<Kind>,
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
    .with_children(shown.adding.map(|kind| create::new_row(ctx, kind)))
    .with_children(groups)
    .with_children(note(ctx, shown.filter));
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
    .padding(Padding::horizontal(SpacingSize::Lg))
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

/// Why every resource of the filter is ready, under the list (`Why` `e1C14P` in `Agents · Instructions` `c3jU1R`): the
/// kinds with nothing to fill in on a computer.
fn note(ctx: &lurq::app::ctx::Ctx, filter: HubFilter) -> Option<Row> {
  let key = match filter {
    HubFilter::Instructions => "team_hub.note.instructions",
    HubFilter::Skills => "team_hub.note.skills",
    _ => return None,
  };
  Some(
    Row::new()
      .id("team-hub-note")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, HubSpacing::NoteY.role()))
      .child(
        Text::new(&ctx.t(key))
          .id("team-hub-note-text")
          .variant(HubText::ListNote.role())
          .flex(1.0),
      ),
  )
}
