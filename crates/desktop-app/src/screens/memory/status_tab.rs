//! A status tab of the project's page: what the tab is and its records. *Waiting for approval* (`05 Memory` `padSP`)
//! says when agents get a record, then lists every proposal of the project and its tasks, oldest first, in one box,
//! each with *Approve*, *Edit* and *Reject* in place. The others have the search and the *Type*, *Author* and *Tag*
//! filters (and on *Active* how many are active and wait) and list the project's records of their statuses, pinned
//! first, each opening its own page, *Active* grouped by type and the rest in one box. A tab without records says so
//! instead, without its search and filters (`ejSHj`); one whose search finds nothing names the tabs that hold matches
//! (`tab_no_match`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
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
  screens::hub::parts::{boxed, group, head as head_row, note},
  theme::{ExtraTypography, HubSpacing, HubText},
  ui::icon::Icon,
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
  let listed: Vec<Memory> = memory
    .pool(statuses)
    .iter()
    .filter(|record| statuses.shows(record.status()))
    .cloned()
    .collect();
  if listed.is_empty() {
    return column.with_children(empty(ctx, state, tab));
  }
  column = column.with_children(head(ctx, tab));
  if tab == MemoryTab::Waiting {
    // The queue is short and oldest first: it lists every proposal, without the search and filters (05).
    return column.with_children(proposals(ctx, state, memory));
  }
  let filters = Filters {
    status: statuses,
    ..state.filters.get()
  };
  let bar = ctx.mount_keyed::<toolbar::FilterBar>(
    "memory-project-filters",
    toolbar::FiltersProps {
      project: true,
      tags: listing::tags(&listed).into(),
    },
  );
  // How many are active and wait, at the toolbar's end on Active (`NW4wY` in 05b).
  let count = (tab == MemoryTab::Active).then(|| {
    Text::new(&ctx.t_args(
      "memory.toolbar.counts",
      [
        ("active", memory.count(listing::StatusFilter::Active).to_string()),
        ("waiting", memory.waiting.len().to_string()),
      ],
    ))
    .id("memory-page-count")
    .variant(HubText::Hint.role())
    .nowrap()
  });
  column = column.child(
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(bar)
      .child(Row::new().flex(1.0))
      .with_children(count),
  );
  match records(ctx, state, (memory, tab), &filters) {
    Some(records) => column.child(records),
    None => column.child(tab_no_match(ctx, state, (memory, tab, shown.order), &filters)),
  }
}

/// What the tab is and how it lists its records; only *Waiting for approval* and *Active* say it.
fn head(ctx: &Ctx, tab: MemoryTab) -> Option<Element> {
  match tab {
    MemoryTab::Waiting => Some(note("memory-tab-note", Icon::Info, &ctx.t("memory.tabs.waitingNote")).into()),
    MemoryTab::Active => Some(
      Row::new()
        .id("memory-tab-head")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t("memory.tabs.activeNote"))
            .id("memory-tab-note")
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .variant(ExtraTypography::Prose.role()),
        )
        .child(
          Text::new(&ctx.t("memory.order"))
            .id("memory-tab-order")
            .variant(TypographyStyle::Caption)
            .nowrap(),
        )
        .into(),
    ),
    MemoryTab::Deprecated | MemoryTab::Superseded | MemoryTab::Rejected | MemoryTab::Settings => None,
  }
}

/// Every proposal, oldest first, in one box under *Waiting for your approval · N · Oldest first*, each with its
/// actions or its review editor in its place (`oTX59`); `None` when none waits.
fn proposals(ctx: &mut Ctx, state: &MemoryState, memory: &ProjectMemory) -> Option<Element> {
  if memory.waiting.is_empty() {
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
  let mut rows = Vec::new();
  for waiting in &memory.waiting {
    rows.push(match opened_on(state.opened.get(), waiting) {
      Some(mode) => {
        let props = opened::editor(ctx, waiting, mode, among);
        ctx.mount_keyed::<RecordEditor>(&format!("memory-editor-{}-{}", waiting.id(), waiting.revision()), props)
      }
      None => proposal::proposal(ctx, (waiting, &scope), &memory.origins, proposal::Listed::Queue),
    });
  }
  let count = ctx.t_args("memory.tabs.queueCount", [("count", memory.waiting.len().to_string())]);
  Some(
    group(
      "memory-queue",
      head_row("memory-queue", &ctx.t("memory.tabs.queueTitle"), Some(&count), None),
      boxed("memory-queue-rows", rows),
    )
    .into(),
  )
}

/// The project's records `filters` show, pinned first: *Active* groups them by type (`Fqhrb`), the other tabs list
/// them without headings (`hxQhj`); `None` when they show none.
fn records(
  ctx: &mut Ctx,
  state: &MemoryState,
  (memory, tab): (&Arc<ProjectMemory>, MemoryTab),
  filters: &Filters,
) -> Option<Element> {
  let input = ListInput {
    records: &memory.records,
    origins: &memory.origins,
    filters,
    task_tab: false,
    project: Some(memory),
  };
  let groups = if tab == MemoryTab::Active {
    list::groups(ctx, state, input)
  } else {
    list::flat(ctx, state, input)
  };
  if groups.is_empty() {
    return None;
  }
  Some(
    Column::new()
      .id("memory-records")
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Groups.role())
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
