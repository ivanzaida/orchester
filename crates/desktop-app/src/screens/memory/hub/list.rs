//! The Memory filter's list (`List` `vjdq8` in Y1, `B7II8x` in Y2, `XzXki` in Y6): what the status means, then its
//! records under their headings — *Waiting for your approval · N · Oldest first* over every proposal of the project
//! and its tasks; *Active* grouped by type, pinned first; the other statuses one group named by the status — and at
//! the list's foot, under the scrolling records, that only a person changes memory (`Person only` `QHE3V`). The
//! records the search hides are left out; records that couldn't be read are said above them, as is why a change of a
//! record the list no longer shows did nothing.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::MemoryId, memory::Memory};
use orchester_workspace::memory::{
  listing::{self, Filters, Group},
  read::ProjectMemory,
};

use super::row::{RowOf, row};
use crate::{
  routes::MemoryTab,
  screens::{
    hub::section::section_row,
    memory::{list::row_failure, parts::unreadable, state::MemoryState, tabs, words},
  },
  theme::{ExtraSpacing, HubText, IconSize},
  ui::icon::{Icon, icon},
};

/// A group of the list: its heading's name in element ids, its words and count, and its records.
pub(super) struct Section<'a> {
  pub name: String,
  pub label: Arc<str>,
  pub count: String,
  pub records: Vec<&'a Memory>,
}

/// The groups `tab` shows of `memory` under `filters`, in their order; none when the search hides every record.
pub(super) fn sections<'a>(
  ctx: &Ctx,
  memory: &'a ProjectMemory,
  tab: MemoryTab,
  filters: &Filters,
) -> Vec<Section<'a>> {
  let records = memory.pool(filters.status);
  match tab {
    MemoryTab::Waiting => {
      let listed = listing::flat(records, filters);
      if listed.is_empty() {
        return Vec::new();
      }
      let count = ctx.t_args("memory.tabs.queueCount", [("count", listed.len().to_string())]);
      vec![Section {
        name: "memory-queue".to_owned(),
        label: ctx.t("memory.tabs.queueTitle"),
        count: count.to_string(),
        records: listed,
      }]
    }
    // Grouped without the waiting ones first, every group is a type's.
    MemoryTab::Active => listing::grouped(records, filters, false)
      .into_iter()
      .filter_map(|listed| match listed.group {
        Group::Kind(kind) => Some(Section {
          name: format!("memory-group-{}", kind.name()),
          label: words::kind_plural(ctx, kind),
          count: listed.records.len().to_string(),
          records: listed.records,
        }),
        Group::Waiting => None,
      })
      .collect(),
    MemoryTab::Deprecated | MemoryTab::Superseded | MemoryTab::Rejected => {
      let listed = listing::flat(records, filters);
      if listed.is_empty() {
        return Vec::new();
      }
      vec![Section {
        name: format!("memory-group-{}", tab.name()),
        label: tabs::name(ctx, tab),
        count: listed.len().to_string(),
        records: listed,
      }]
    }
  }
}

/// What the list shows.
pub(super) struct Shown<'a> {
  pub memory: &'a Arc<ProjectMemory>,
  pub tab: MemoryTab,
  pub filters: &'a Filters,
  pub sections: &'a [Section<'a>],
  /// The record the panel shows.
  pub chosen: Option<MemoryId>,
}

pub(super) fn list(ctx: &mut Ctx, state: &MemoryState, shown: &Shown<'_>) -> Element {
  let latest = (shown.tab == MemoryTab::Active)
    .then(|| listing::latest_handoff(&shown.memory.records))
    .flatten();
  let mut column = Column::new()
    .id("memory-records")
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(ExtraSpacing::Snug.role()))
    .with_children(note(ctx, shown.tab))
    .with_children((shown.memory.unreadable > 0).then(|| inset(unreadable(ctx, shown.memory.unreadable))))
    .with_children(row_failure(ctx, state, &shown.memory.records, shown.filters).map(inset));
  for section in shown.sections {
    column = column.child(section_row(
      &section.name,
      &section.label,
      (&section.count, PaletteColor::TextMuted),
    ));
    for record in &section.records {
      let of = RowOf {
        tab: shown.tab,
        chosen: Some(record.id()) == shown.chosen,
        latest: Some(record.id()) == latest,
        origins: &shown.memory.origins,
      };
      column = column.child(row(ctx, state, record, &of));
    }
  }
  Column::new()
    .id("memory-list")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(
      ScrollVertical::new(column)
        .id("memory-list-scroll")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0)),
    )
    .child(person_only(ctx))
    .into()
}

/// What the status means, over its records (`Note` `Iain8` in Y1, `Kxqp5` in Y2, `n2zpC` in Y6); Superseded and
/// Rejected have none.
fn note(ctx: &Ctx, tab: MemoryTab) -> Option<Element> {
  let key = match tab {
    MemoryTab::Waiting => "memory.tabs.waitingNote",
    MemoryTab::Active => "memory.tabs.activeNote",
    MemoryTab::Deprecated => "memory.tabs.deprecatedNote",
    MemoryTab::Superseded | MemoryTab::Rejected => return None,
  };
  Some(
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Sm)
          .bottom(ExtraSpacing::Snug.role())
          .left(SpacingSize::Lg)
          .right(SpacingSize::Lg),
      )
      .child(
        Text::new(&ctx.t(key))
          .id("memory-tab-note")
          .variant(HubText::ListNote.role())
          .flex(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .into(),
  )
}

/// A notice in the list, 16 in from its sides.
fn inset(notice: Element) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Sm))
    .child(notice)
    .into()
}

/// *Only a person changes memory here: …* at the list's foot (`QHE3V`): a 13 `user-check` and the words in 12/1.45
/// `text_muted`, 8 apart, 12 over and under them and 16 at the sides.
fn person_only(ctx: &Ctx) -> Element {
  Row::new()
    .id("memory-person-only")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(Icon::UserRoundCheck, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t("memory.personOnly"))
        .id("memory-person-only-text")
        .variant(HubText::ListNote.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}
