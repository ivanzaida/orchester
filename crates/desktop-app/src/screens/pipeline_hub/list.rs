//! The hub's list (`List` `l5LJYx` in `Pipelines · v2` P1): the pipelines of the tab in two groups by whether they can
//! run as they are (*Needs you*, *Ready*), each under its title with a count, or under *Archived* one list of what was
//! archived.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, ScrollVertical},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::readiness::Group;

use super::{
  model::{Entry, GROUPS, Listing},
  row::row,
};
use crate::{
  navigation::Navigation,
  routes::PipelinesFilter,
  screens::{hub::section::section, team_hub::list::LIST_WIDTH},
  theme::ExtraSpacing,
};

/// What the list needs besides the entries.
pub(super) struct Shown<'a> {
  pub project: ProjectId,
  pub filter: PipelinesFilter,
  pub selected: Option<orchester_core::id::ResourceId>,
  pub navigation: &'a Navigation,
  pub now: Timestamp,
}

pub(super) fn list(ctx: &Ctx, listing: &Listing, shown: &Shown<'_>) -> Element {
  let sections: Vec<Column> = if shown.filter == PipelinesFilter::Archived {
    let archived: Vec<&Entry> = listing.shown(shown.filter).collect();
    vec![section_of(
      ctx,
      ("archived", "pipeline_hub.group.archived", PaletteColor::TextMuted),
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
    .id("pipeline-hub-list")
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .with_children(sections);
  ScrollVertical::new(groups)
    .id("pipeline-hub-list-scroll")
    .width(Dimension::Px(LIST_WIDTH))
    .height(Dimension::Pct(100.0))
    .into()
}

fn section_of(ctx: &Ctx, name: (&str, &str, PaletteColor), entries: Vec<&Entry>, shown: &Shown<'_>) -> Column {
  Column::new()
    .width(Dimension::Pct(100.0))
    .child(section(ctx, "pipeline-hub", name, entries.len()))
    .with_children(entries.into_iter().map(|entry| row(ctx, entry, shown)))
}

const fn named(group: Group) -> (&'static str, &'static str, PaletteColor) {
  match group {
    Group::NeedsYouHere => ("needs", "pipeline_hub.group.needs", PaletteColor::Warning),
    Group::Ready | Group::OffHere => ("ready", "pipeline_hub.group.ready", PaletteColor::TextMuted),
  }
}
