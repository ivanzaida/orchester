//! The hub's list in a local project (`List` in `Agents · v2` L2): the entries of the filter in three groups by what
//! the person has to do (*Needs you*, *Ready*, *Off*), each under its title with a count, or under *Archived* one list
//! of what was archived, each row with its kind and when (`Agents · Archived` L13).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::{local_archive::Archivable, readiness::Group};

use super::{
  leave::Leave,
  model::{Entry, GROUPS, Listing},
  row::row,
  words::Unsaved,
};
use crate::{
  routes::HubFilter,
  screens::team_hub::{create::new_row, list::LIST_WIDTH, model::Kind},
  theme::{ExtraSpacing, HubText},
};

/// A group's title row.
const SECTION_HEIGHT: f32 = 32.0;

/// What the list needs besides the entries.
pub(super) struct Shown<'a> {
  pub project: ProjectId,
  pub filter: HubFilter,
  pub selected: Option<Archivable>,
  pub leave: &'a Leave,
  pub now: Timestamp,
  /// The MCP server whose panel has edits that are not saved, and whether a save was refused.
  pub unsaved: Option<(Archivable, Unsaved)>,
  /// The kind whose form is open in the panel: its *New* row stands over the groups, chosen.
  pub adding: Option<Kind>,
}

pub(super) fn list(ctx: &Ctx, listing: &Listing, shown: &Shown<'_>) -> Element {
  let sections: Vec<Column> = if shown.filter == HubFilter::Archived {
    let archived: Vec<&Entry> = listing.shown(shown.filter).collect();
    vec![section_of(
      ctx,
      ("archived", "local_hub.group.archived", PaletteColor::TextMuted),
      archived,
      shown,
    )]
  } else {
    GROUPS
      .into_iter()
      .map(|group| (group, listing.group(shown.filter, group).collect::<Vec<_>>()))
      .filter(|(_, entries)| !entries.is_empty())
      .map(|(group, entries)| section_of(ctx, named(group), entries, shown))
      .collect()
  };
  let groups = Column::new()
    .id("local-hub-list")
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .with_children(shown.adding.map(|kind| new_row(ctx, kind)))
    .with_children(sections);
  ScrollVertical::new(groups)
    .id("local-hub-list-scroll")
    .width(Dimension::Px(LIST_WIDTH))
    .height(Dimension::Pct(100.0))
    .into()
}

fn section_of(ctx: &Ctx, name: (&str, &str, PaletteColor), entries: Vec<&Entry>, shown: &Shown<'_>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(section(ctx, name, entries.len()))
    .with_children(entries.into_iter().map(|entry| row(ctx, entry, shown)))
}

const fn named(group: Group) -> (&'static str, &'static str, PaletteColor) {
  match group {
    Group::NeedsYouHere => ("needs", "local_hub.group.needs", PaletteColor::Warning),
    Group::Ready => ("ready", "local_hub.group.ready", PaletteColor::TextMuted),
    Group::OffHere => ("off", "local_hub.group.off", PaletteColor::TextMuted),
  }
}

/// A group's title row: its name in element ids, its words' key and the color of its count.
fn section(ctx: &Ctx, (name, key, color): (&str, &str, PaletteColor), count: usize) -> Row {
  section_row(
    &format!("local-hub-section-{name}"),
    &ctx.t(key),
    (&count.to_string(), color),
  )
}

/// A group's title row in the hub's list (`Section · Waiting` `pODwY` in Y1): `label` in 12/600 `text_secondary` and
/// `count` in 12 `color`, 8 apart, 32 high and 16 in; `id` names it, `<id>-title` and `<id>-count` its words.
pub(in crate::screens) fn section_row(id: &str, label: &str, (count, color): (&str, PaletteColor)) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SECTION_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Lg))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(label)
        .id(format!("{id}-title"))
        .variant(HubText::GroupTitle.role())
        .nowrap(),
    )
    .child(
      Text::new(count)
        .id(format!("{id}-count"))
        .variant(HubText::Hint.role())
        .color(color)
        .nowrap(),
    )
}
