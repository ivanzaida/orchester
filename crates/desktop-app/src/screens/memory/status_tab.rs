//! A status tab of the project's page: what the tab is (*Waiting for approval* `oX9TR` in `CIThk`, *Active* `UZ2TJ`
//! in `TeKGM`), the search and the *Type*, *Author* and *Tag* filters, and its records. *Waiting for approval* lists
//! every proposal of the project and its tasks, oldest first, each with *Reject*, *Edit* and *Approve* in place; the
//! others list the project's records of their statuses grouped by type, pinned first, each opening its details in
//! place. A tab without records says so instead, without its search and filters (`ejSHj`); one whose search finds
//! nothing names the tabs that hold matches (`tab_no_match`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::memory::Memory;
use orchester_workspace::memory::{
  listing::{self, Filters},
  read::ProjectMemory,
};

use super::{
  centered::Centered,
  editor::{RecordEditor, opened_on},
  list::{self, ListInput},
  opened, proposal,
  state::MemoryState,
  tab_no_match::tab_no_match,
  tabs::{self, Order},
  toolbar,
};
use crate::{
  routes::MemoryTab,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::icon::{Icon, icon},
};

/// What a status tab shows: `tab` of `memory`, the page's tabs in `order`.
#[derive(Clone, Copy)]
pub struct Shown<'a> {
  pub memory: &'a Arc<ProjectMemory>,
  pub tab: MemoryTab,
  pub order: Order,
}

/// The body of a status tab, with *New record*'s editor and the unreadable records' notice above it.
pub fn body(ctx: &mut Ctx, state: &MemoryState, shown: Shown<'_>) -> Column {
  let Shown { memory, tab, .. } = shown;
  let mut column = Column::new()
    .id(format!("memory-tab-body-{}", tab.name()))
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md);
  column = column.with_children(list::new_record(ctx, state));
  if memory.unreadable > 0 {
    column = column.child(super::unreadable(ctx, memory.unreadable));
  }
  let Some(statuses) = tabs::statuses(tab) else {
    return column;
  };
  let listed: Vec<Memory> = tabs::pool(tab, memory)
    .iter()
    .filter(|record| statuses.shows(record.status()))
    .cloned()
    .collect();
  if listed.is_empty() {
    return column.with_children(empty(ctx, state, tab));
  }
  let filters = Filters {
    status: statuses,
    ..state.filters.get()
  };
  column = column.with_children(head(ctx, tab));
  column = column.child(ctx.mount_keyed::<toolbar::FilterBar>(
    "memory-project-filters",
    toolbar::FiltersProps {
      project: true,
      tags: listing::tags(&listed).into(),
    },
  ));
  let records = if tab == MemoryTab::Waiting {
    proposals(ctx, state, memory, &filters)
  } else {
    records(ctx, state, memory, &filters)
  };
  match records {
    Some(records) => column.child(records),
    None => column.child(tab_no_match(ctx, state, (memory, tab, shown.order), &filters)),
  }
}

/// What the tab is and how it lists its records; only *Waiting for approval* and *Active* say it.
fn head(ctx: &Ctx, tab: MemoryTab) -> Option<Row> {
  let (glyph, note, order) = match tab {
    MemoryTab::Waiting => (Some(Icon::Hand), "memory.tabs.waitingNote", "memory.tabs.oldestFirst"),
    MemoryTab::Active => (None, "memory.tabs.activeNote", "memory.order"),
    MemoryTab::Deprecated | MemoryTab::Superseded | MemoryTab::Rejected | MemoryTab::Settings => return None,
  };
  Some(
    Row::new()
      .id("memory-tab-head")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(glyph.map(|glyph| icon(glyph, IconSize::Md, PaletteColor::Warning)))
      .child(
        Text::new(&ctx.t(note))
          .id("memory-tab-note")
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .variant(ExtraTypography::Prose.role()),
      )
      .child(
        Text::new(&ctx.t(order))
          .id("memory-tab-order")
          .variant(TypographyStyle::Caption)
          .nowrap(),
      ),
  )
}

/// Every proposal `filters` show, oldest first, each with its actions or its review editor in its place
/// (`PmgBN`); `None` when they show none.
fn proposals(ctx: &mut Ctx, state: &MemoryState, memory: &ProjectMemory, filters: &Filters) -> Option<Element> {
  let shown: Vec<&Memory> = memory.waiting.iter().filter(|record| filters.shows(record)).collect();
  if shown.is_empty() {
    return None;
  }
  let mut scope: Vec<Memory> = memory.records.clone();
  scope.extend(memory.replaced.iter().cloned());
  let among = opened::Among {
    records: &scope,
    origins: &memory.origins,
    with_task: true,
    project: Some(memory),
  };
  let mut list = Column::new()
    .id("memory-queue")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role());
  for waiting in shown {
    let entry = match opened_on(state.opened.get(), waiting) {
      Some(mode) => {
        let props = opened::editor(ctx, waiting, mode, among);
        ctx.mount_keyed::<RecordEditor>(&format!("memory-editor-{}-{}", waiting.id(), waiting.revision()), props)
      }
      None => proposal::proposal(ctx, (waiting, &scope), &memory.origins, proposal::Listed::Queue),
    };
    list = list.child(entry);
  }
  Some(list.into())
}

/// The project's records `filters` show, grouped by type, pinned first (`Fqhrb`); `None` when they show none.
fn records(ctx: &mut Ctx, state: &MemoryState, memory: &Arc<ProjectMemory>, filters: &Filters) -> Option<Element> {
  let input = ListInput {
    records: &memory.records,
    origins: &memory.origins,
    filters,
    task_tab: false,
    project: Some(memory),
  };
  let groups = list::groups(ctx, state, input);
  if groups.is_empty() {
    return None;
  }
  Some(
    Column::new()
      .id("memory-records")
      .width(Dimension::Pct(100.0))
      .with_children(list::row_failure(ctx, state, &memory.records, filters))
      .with_children(groups)
      .into(),
  )
}

/// A status tab without records (`xFUjH` in `ejSHj`, `qfF5t` in `C0OJpC`).
fn empty(ctx: &Ctx, state: &MemoryState, tab: MemoryTab) -> Option<Element> {
  let (title, body) = match tab {
    MemoryTab::Waiting => ("memory.tabs.empty.waitingTitle", "memory.tabs.empty.waitingBody"),
    MemoryTab::Active => ("memory.tabs.empty.activeTitle", "memory.tabs.empty.activeBody"),
    MemoryTab::Deprecated => ("memory.tabs.empty.deprecatedTitle", "memory.tabs.empty.deprecatedBody"),
    MemoryTab::Superseded => ("memory.tabs.empty.supersededTitle", "memory.tabs.empty.supersededBody"),
    MemoryTab::Rejected => ("memory.tabs.empty.rejectedTitle", "memory.tabs.empty.rejectedBody"),
    MemoryTab::Settings => return None,
  };
  Some(
    Centered::new(
      "memory-empty",
      tabs::glyph(tab),
      ctx.t(title),
      ctx.t_args(body, [("project", &*state.project)]),
    )
    .empty_tab()
    .into(),
  )
}
