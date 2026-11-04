//! A status tab of the project's page: what the tab is and its records. *Waiting for approval* (`05 Memory` `padSP`)
//! says when agents get a record, then lists every proposal of the project and its tasks, oldest first, in one box,
//! each with *Approve*, *Edit* and *Reject* in place. *Active* (05b) has the search and its *Filter* with how many are
//! active and wait, then what reaches agents, then its records grouped by type; the others (05f) list their records in
//! one box, *Deprecated* with a note under them. Records are pinned first, each opening its own page. A tab without
//! records says so instead in one quiet row, without its search and filters (06b `VcE5a`); one whose search finds
//! nothing names the tabs that hold matches (`tab_no_match`).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
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
  editor::{RecordEditor, opened_on},
  list::{self, ListInput},
  opened, proposal,
  state::{MemoryState, Opened},
  tab_no_match::tab_no_match,
  tabs::{self, Order},
  toolbar,
};
use crate::{
  routes::MemoryTab,
  screens::hub::parts::{Entry, boxed, group, head as head_row, note},
  theme::{HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
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
  let statuses = tabs::statuses(tab);
  let listed: Vec<Memory> = memory
    .pool(statuses)
    .iter()
    .filter(|record| statuses.shows(record.status()))
    .cloned()
    .collect();
  if listed.is_empty() {
    keep_bar(state, None);
    return column.child(empty(ctx, state, tab));
  }
  if tab == MemoryTab::Waiting {
    keep_bar(state, None);
    // The queue is short and oldest first: it lists every proposal, without the search and filters (05).
    return column
      .child(note("memory-tab-note", Icon::Info, &ctx.t("memory.tabs.waitingNote")))
      .with_children(proposals(ctx, state, memory));
  }
  let filters = Filters {
    status: statuses,
    ..state.filters.get()
  };
  // Active has the search and its Filter (05b); the other tabs list their records without them (05f), unless a search
  // or a filter brought the person there, which they can then clear.
  let narrowed = filters
    != Filters {
      status: statuses,
      ..Filters::default()
    };
  let kept = keep_bar(state, (tab != MemoryTab::Active).then_some((tab, narrowed)));
  if tab == MemoryTab::Active || kept {
    column = column.child(bar(ctx, (memory, &listed), tab));
  }
  if tab == MemoryTab::Active {
    column = column.child(note("memory-tab-note", Icon::Info, &ctx.t("memory.tabs.activeNote")));
  }
  match records(ctx, state, (memory, tab), &filters) {
    Some(records) => column.child(records).with_children(
      (tab == MemoryTab::Deprecated).then(|| note("memory-tab-foot", Icon::Info, &ctx.t("memory.tabs.deprecatedNote"))),
    ),
    None => column.child(tab_no_match(ctx, state, (memory, tab, shown.order), &filters)),
  }
}

/// Whether a tab other than Active shows its search and Filter: from when a search or a filter narrows it (`shown`:
/// the tab and whether its filters narrow it) until another tab shows, so clearing the search's last character keeps
/// the field and its focus. `None` for a tab without them.
fn keep_bar(state: &MemoryState, shown: Option<(MemoryTab, bool)>) -> bool {
  let stored = state.bar_kept.get_untracked();
  let kept = shown.and_then(|(tab, narrowed)| (narrowed || stored == Some(tab)).then_some(tab));
  if kept != stored {
    state.bar_kept.set(kept);
  }
  kept.is_some()
}

/// The search and the Filter, with how many are active and wait at the end on Active (`E3atNZ`, `NW4wY` in 05b).
fn bar(ctx: &mut Ctx, (memory, listed): (&ProjectMemory, &[Memory]), tab: MemoryTab) -> Element {
  let bar = ctx.mount_keyed::<toolbar::FilterBar>(
    "memory-project-filters",
    toolbar::FiltersProps {
      project: true,
      tags: listing::tags(listed).into(),
    },
  );
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
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(bar)
    .child(Row::new().flex(1.0))
    .with_children(count)
    .into()
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

/// A status tab without records: one quiet row in its box, Active's with *New record* (`KVUQx`, `Och5l` in 06b
/// `VcE5a`).
fn empty(ctx: &Ctx, state: &MemoryState, tab: MemoryTab) -> Element {
  let (title, body) = match tab {
    MemoryTab::Waiting => ("memory.tabs.empty.waitingTitle", "memory.tabs.empty.waitingBody"),
    MemoryTab::Active => ("memory.tabs.empty.activeTitle", "memory.tabs.empty.activeBody"),
    MemoryTab::Deprecated => ("memory.tabs.empty.deprecatedTitle", "memory.tabs.empty.deprecatedBody"),
    MemoryTab::Superseded => ("memory.tabs.empty.supersededTitle", "memory.tabs.empty.supersededBody"),
    MemoryTab::Rejected => ("memory.tabs.empty.rejectedTitle", "memory.tabs.empty.rejectedBody"),
  };
  let mut row = Entry::new("memory-empty", ctx.t(title))
    .tile(tabs::glyph(tab))
    .hint(ctx.t_args(body, [("project", &*state.project)]));
  if tab == MemoryTab::Active {
    let open = state.clone();
    row = row.end(
      ActionButton::new("memory-empty-new", ButtonKind::Secondary, ctx.t("memory.newRecord"))
        .icon(Icon::Plus)
        .on_click(move || open.open(Opened::New)),
    );
  }
  boxed("memory-empty-rows", [row.into()]).into()
}
