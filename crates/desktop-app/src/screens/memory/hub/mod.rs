//! The Agents hub's Memory filter (`Section Y · Memory` `Xxl1u` in `Agents · v2` `acFkz`; notes Question B `lmKu1`
//! and Rule 2 `Yfo18`): what this project's agents remember across tasks, kept on this computer, in the hub's frame
//! after Tools. Under the hub's filter its statuses are a sub-filter with the search and *Memory settings ↗*
//! ([`filter`]); the body is the hub's list of the chosen status's records ([`list`]) and the panel of the picked one,
//! with its actions pinned at the bottom ([`panel`]); a record being reviewed, edited or written takes the panel as its
//! editor, and *Delete…* asks in a modal. A status without records says so across the body (Y4 `i7aEec`), and one
//! whose search finds nothing names the statuses that hold matches. The status and the record are the route's
//! ([`MemoryView`]); without a record the panel shows the first of the list.
//!
//! Memory is never team configuration: in a team project the filter shows this computer's memory of the project and
//! says so under its sub-filter (Y5 `Fnu7A`); nothing of it is sent to the team server. Every change happens through
//! the workspace's memory rules (`orchester_workspace::memory`), off the UI thread, and only a person makes it: input
//! from an MCP client is refused where the change was asked for.

mod filter;
mod list;
pub(in crate::screens::memory) mod panel;
mod pills;
mod read;
mod row;

use std::sync::{Arc, OnceLock};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::memory::Memory;
use orchester_workspace::{
  memory::{listing::Filters, read::ProjectMemory},
  messages::Message,
};
use parking_lot::Mutex;

pub(in crate::screens) use self::read::active_count;
use self::{
  panel::{PanelProps, RecordPanel},
  read::Read,
};
use super::{
  delete::DeleteConfirm,
  editor::{EditorMode, RecordEditor},
  opened::{self, Among},
  state::{MemoryState, Opened},
  tab_no_match::tab_no_match,
  tabs::{self, Order},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{MemoryTab, MemoryView},
  screens::hub::{agents, agents_bones, new_button},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    failure::{Failure, LastFailure},
    icon::Icon,
  },
};

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  view: MemoryView,
  /// The project is a team's.
  team: bool,
}

impl DevtoolsInspectable for Props {}

/// The filter's body under the hub's filter, on `view`; the hub provides its [`MemoryState`] first.
pub(in crate::screens) fn body(ctx: &mut Ctx, project: &ShownProject, view: MemoryView, team: bool) -> Element {
  ctx.mount_keyed::<MemoryHub>(
    &format!("memory-hub-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      view,
      team,
    },
  )
}

/// *New record* in the hub's head, in the slot of the hub's *New*: it opens the editor in the panel on *Active*.
pub(in crate::screens) fn new_record(ctx: &mut Ctx, state: &MemoryState, view: MemoryView) -> Element {
  let (open, navigation, project) = (state.clone(), Navigation::of(ctx), state.project_id);
  let button = ActionButton::new("memory-new", ButtonKind::Primary, ctx.t("memory.newRecord"))
    .icon(Icon::Plus)
    .on_click(move || {
      open.open(Opened::New);
      if view.tab != Some(MemoryTab::Active) || view.record.is_some() {
        navigation.go(tabs::route(project, MemoryTab::Active));
      }
    });
  new_button::slot(ctx, "memory-new", button)
}

struct MemoryHub {
  retries: Signal<u64>,
  /// The order of the statuses, set by the first reading: it never moves while the filter shows.
  order: OnceLock<Order>,
  /// The status shown last; another closes what was open in place of a record.
  shown: Mutex<Option<MemoryTab>>,
  /// Why the memory couldn't be read, kept while it is read again.
  failure: LastFailure,
}

impl Component for MemoryHub {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      retries: ctx.signal(0),
      order: OnceLock::new(),
      shown: Mutex::new(None),
      failure: LastFailure::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, view, team } = ctx.props::<Props>().clone();
    let state = MemoryState::of(ctx);
    let memory = match read::memory(ctx, &project, (self.retries.get(), &self.failure)) {
      Read::Ready(memory) => memory,
      Read::Failed(error, retrying) => {
        state.follow_read::<ProjectMemory>(None);
        return self.failure_page(ctx, &project, error, retrying);
      }
      Read::Reading => {
        // Until the first reading the statuses keep the order they would have while proposals wait.
        let order = self.order.get().copied().unwrap_or(Order::of(1));
        let tab = view.tab.unwrap_or(order.opening());
        let strip = filter::sub_filter(ctx, &state, (None, order), tab);
        let bones = agents_bones::body(ctx, "memory-loading", ctx.t("memory.reading"));
        return frame(ctx, (&project, team, tab), (strip, bones));
      }
    };
    state.follow_read(Some(&memory));
    let order = *self.order.get_or_init(|| Order::of(memory.waiting.len()));
    let tab = view.tab.unwrap_or(order.opening());
    if self.shown.lock().replace(tab).is_some_and(|last| last != tab)
      && !matches!(state.opened.get_untracked(), Opened::Nothing | Opened::New)
    {
      state.close();
    }
    let strip = filter::sub_filter(ctx, &state, (Some(&memory), order), tab);
    let body = shown(ctx, &state, (&memory, order, tab), (view, team));
    frame(ctx, (&project, team, tab), (strip, body))
  }
}

impl MemoryHub {
  /// *Memory couldn't be read*, why, *Try again* and the runner's log, in place of the sub-filter and the body.
  fn failure_page(&self, ctx: &mut Ctx, project: &ShownProject, error: Message, retrying: bool) -> Element {
    let again = self.retries.clone();
    Failure::new("memory-load-error", ctx.t("memory.loadFailed"), error)
      .retry(retrying, move || again.update(|count| *count += 1))
      .log(project.open.repo_root.clone())
      .still_works(ctx.t("memory.loadStillWorks"))
      .page(ctx)
  }
}

/// The sub-filter, the line that the memory is this computer's in a team project, and `tab`'s body under them.
fn frame(
  ctx: &Ctx,
  (project, team, tab): (&ShownProject, bool, MemoryTab),
  (strip, body): (Element, Element),
) -> Element {
  Column::new()
    .id("memory-body")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inset.role())
    .child(strip)
    .with_children(team.then(|| filter::not_shared(ctx, project.open.project.name().as_str())))
    .child(
      Column::new()
        .id(format!("memory-tab-body-{}", tab.name()))
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(body),
    )
    .into()
}

/// The list and the panel of `tab`, or what stands for them when the status holds no record or its search finds none.
fn shown(
  ctx: &mut Ctx,
  state: &MemoryState,
  (memory, order, tab): (&Arc<ProjectMemory>, Order, MemoryTab),
  (view, team): (MemoryView, bool),
) -> Element {
  let filters = Filters {
    status: tabs::statuses(tab),
    ..state.filters.get()
  };
  let sections = list::sections(ctx, memory, tab, &filters);
  let listed: Vec<&Memory> = sections
    .iter()
    .flat_map(|section| section.records.iter().copied())
    .collect();
  let chosen = view
    .record
    .filter(|record| listed.iter().any(|listed| listed.id() == *record))
    .or_else(|| listed.first().map(|record| record.id()));
  // *New record*'s editor takes the panel, beside the list, whatever it lists.
  if chosen.is_none() && state.opened.get() != Opened::New {
    return nothing_listed(ctx, state, (memory, order, tab), &filters);
  }
  master_detail(ctx, state, (memory, tab, &filters, &sections), chosen, team)
}

/// The list beside the panel of `chosen`, or of the editor open in its place.
fn master_detail(
  ctx: &mut Ctx,
  state: &MemoryState,
  (memory, tab, filters, sections): (&Arc<ProjectMemory>, MemoryTab, &Filters, &[list::Section<'_>]),
  chosen: Option<orchester_core::id::MemoryId>,
  team: bool,
) -> Element {
  // While *New record* is written no row is the panel's.
  let picked = chosen.filter(|_| state.opened.get() != Opened::New);
  let list = list::list(
    ctx,
    state,
    &list::Shown {
      memory,
      tab,
      filters,
      sections,
      chosen: picked,
    },
  );
  let record = chosen.and_then(|chosen| {
    memory
      .waiting
      .iter()
      .chain(&memory.records)
      .find(|record| record.id() == chosen)
  });
  let panel = panel_of(ctx, state, memory, record, team);
  let mut row = agents::master_detail("memory", false, (list, panel));
  if let Some(record) = record.filter(|record| state.opened.get() == Opened::Delete(record.id())) {
    let props = opened::delete(record, among(memory, &scope(memory)));
    let modal = ctx.mount_keyed::<DeleteConfirm>(&format!("memory-delete-{}", record.id()), props);
    row = Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(row)
      .child(modal)
      .into();
  }
  row
}

/// The panel: *New record*'s editor while it is open, the review or the edit of `record` while one is, else what
/// `record` is.
fn panel_of(
  ctx: &mut Ctx,
  state: &MemoryState,
  memory: &Arc<ProjectMemory>,
  record: Option<&Memory>,
  team: bool,
) -> Element {
  let opened = state.opened.get();
  if opened == Opened::New {
    let props = opened::new_record(ctx, &state.project);
    return ctx.mount_keyed::<RecordEditor>("memory-editor-new", props);
  }
  let Some(record) = record else {
    return Element::new();
  };
  let mode = match opened {
    Opened::Review(id) if id == record.id() => Some(EditorMode::Review),
    Opened::Edit(id) if id == record.id() => Some(EditorMode::Edit),
    _ => None,
  };
  if let Some(mode) = mode {
    let scope = scope(memory);
    let props = opened::editor(ctx, record, mode, among(memory, &scope));
    return ctx.mount_keyed::<RecordEditor>(&format!("memory-editor-{}-{}", record.id(), record.revision()), props);
  }
  ctx.mount_keyed::<RecordPanel>(
    "memory-panel",
    PanelProps {
      memory: Arc::clone(memory),
      record: record.id(),
      team,
    },
  )
}

/// The records an editor or a confirmation is opened among: the project's, and those its proposals would replace.
fn scope(memory: &ProjectMemory) -> Vec<Memory> {
  let mut scope = memory.records.clone();
  scope.extend(memory.replaced.iter().cloned());
  scope
}

fn among<'a>(memory: &'a ProjectMemory, scope: &'a [Memory]) -> Among<'a> {
  Among {
    records: scope,
    origins: &memory.origins,
    with_task: true,
    project: Some(memory),
  }
}

/// A status that lists nothing: one without records says so across the body with its glyph, *Active* with *New
/// record* (Y4 `i7aEec`); one whose search hides every record says what matches elsewhere.
fn nothing_listed(
  ctx: &mut Ctx,
  state: &MemoryState,
  (memory, order, tab): (&ProjectMemory, Order, MemoryTab),
  filters: &Filters,
) -> Element {
  if memory.count(filters.status) > 0 {
    let found_elsewhere = tab_no_match(ctx, state, (memory, tab, order), filters);
    return agents::framed("memory-no-match-body", found_elsewhere);
  }
  let (title, body) = match tab {
    MemoryTab::Waiting => ("memory.tabs.empty.waitingTitle", "memory.tabs.empty.waitingBody"),
    MemoryTab::Active => ("memory.tabs.empty.activeTitle", "memory.tabs.empty.activeBody"),
    MemoryTab::Deprecated => ("memory.tabs.empty.deprecatedTitle", "memory.tabs.empty.deprecatedBody"),
    MemoryTab::Superseded => ("memory.tabs.empty.supersededTitle", "memory.tabs.empty.supersededBody"),
    MemoryTab::Rejected => ("memory.tabs.empty.rejectedTitle", "memory.tabs.empty.rejectedBody"),
  };
  let more = (tab == MemoryTab::Active).then(|| {
    let open = state.clone();
    ActionButton::new("memory-empty-new", ButtonKind::Secondary, ctx.t("memory.newRecord"))
      .icon(Icon::Plus)
      .on_click(move || open.open(Opened::New))
      .into()
  });
  agents::empty(
    "memory",
    tabs::glyph(tab),
    (&ctx.t(title), &ctx.t_args(body, [("project", &*state.project)])),
    more.into_iter().collect(),
  )
}
