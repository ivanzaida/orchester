//! Memory: the project's page (`Project memory` `x4pFb`, `Memory with tabs` `wMr98`) and the parts the task's Memory
//! tab shares with it (`Artifacts & Memory` `g5dmWI`). A record's row, a proposal, the record editor and the delete
//! confirmation are one set of parts; every change happens in place, through the workspace's memory rules
//! (`orchester_workspace::memory`), off the UI thread.
//!
//! The project's page: the title with *New record*, then a tab per status of its records — *Waiting for approval*,
//! every proposal of the project and its tasks; *Active*, *Deprecated*, *Superseded* and *Rejected*, the project's own
//! records searched, filtered and grouped by type, each opening its details in place — and *Settings*, how agents'
//! records are saved and what each agent gets. The tab is part of the route (`ProjectPage::Memory`).

mod centered;
mod delete;
mod details;
mod editor;
mod list;
mod no_match;
mod opened;
mod page;
mod parts;
mod proposal;
mod row;
mod settings;
pub(crate) mod state;
mod status_tab;
mod tab_no_match;
mod tabs;
mod toolbar;
pub(crate) mod words;
mod work;

use std::sync::OnceLock;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_workspace::{
  background,
  memory::{listing::Filters, read::ProjectMemory},
  messages::{self, Message},
};
use parking_lot::Mutex;

pub(crate) use self::{
  centered::Centered,
  list::{ListInput, groups, new_record, row_failure},
  no_match::no_match,
  page::unreadable,
  parts::{group_heading, unbuilt_link},
  toolbar::{FilterBar, FiltersProps},
};
use self::{state::Made, tabs::Order};
use crate::{project::ShownProject, routes::MemoryTab, services::Services};

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  /// The tab the route names; `None` for the one the page opens on.
  tab: Option<MemoryTab>,
}

impl DevtoolsInspectable for Props {}

/// The project's Memory page on `tab`. Its search, filters and the order of its tabs stay while another tab of it
/// opens.
pub fn screen(ctx: &mut Ctx, project: &ShownProject, tab: Option<MemoryTab>) -> Element {
  ctx.mount_keyed::<MemoryScreen>(
    &format!("memory-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      tab,
    },
  )
}

struct MemoryScreen {
  made: Made,
  retries: lurq::core::Signal<u64>,
  /// The order of the tabs, set by the first reading: it never moves while the page shows.
  order: OnceLock<Order>,
  /// The tab shown last; opening another closes the details open in place.
  shown: Mutex<Option<MemoryTab>>,
}

impl Component for MemoryScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let project = ctx.props::<Props>().project.clone();
    Self {
      made: Made::new(ctx, Filters::default(), &project.feed),
      retries: ctx.signal(0),
      order: OnceLock::new(),
      shown: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, tab } = ctx.props::<Props>().clone();
    let state = self.made.state(ctx, &project, None);
    ctx.provide(state.clone());
    let read = read(ctx, &project, self.retries.get());
    let view = match &read {
      Read::Ready(memory) => Some(*self.order.get_or_init(|| Order::of(memory.waiting.len()))),
      Read::Reading | Read::Failed(_) => self.order.get().copied(),
    }
    .map(|order| page::view(order, tab));
    if let Some(view) = view
      && self.shown.lock().replace(view.tab).is_some_and(|last| last != view.tab)
    {
      state.selected.set(state::Chosen::default());
    }
    page::page(ctx, &state, (&project, read, &self.retries), view)
  }
}

/// What the page shows of the project's memory, read off the UI thread whenever the project's tasks are saved or its
/// runs change, and when the person tries again.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Read {
  Reading,
  Ready(std::sync::Arc<ProjectMemory>),
  Failed(Message),
}

fn read(ctx: &mut Ctx, project: &ShownProject, retries: u64) -> Read {
  let feed = &project.feed;
  let again = feed.revision.get() + feed.run_changes(feed.running.get()) + retries;
  let hold = Services::of(ctx).hold;
  let loaded = ctx
    .future((project.open.repo_root.clone(), again), move |(repo, _)| {
      let hold = hold.clone();
      async move {
        hold.released().await;
        background::run(move || ProjectMemory::load_in(&repo).map(state::Shared::new))
          .await?
          .map_err(|error| messages::store_error(&error))
      }
    })
    .state()
    .get();
  // Data kept from before a failed reread is not shown as current.
  if let Some(error) = loaded.error.clone().filter(|_| loaded.is_rejected()) {
    return Read::Failed(error);
  }
  loaded.data.map_or(Read::Reading, |read| Read::Ready(read.0))
}
