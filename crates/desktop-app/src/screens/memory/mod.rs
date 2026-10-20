//! Memory: the project's page (`Project memory` `x4pFb`) and the parts the task's Memory tab shares with it
//! (`Artifacts & Memory` `g5dmWI`). A record's row, a proposal, the record editor and the delete confirmation are
//! one set of parts; every change happens in place, through the workspace's memory rules
//! (`orchester_workspace::memory`), off the UI thread.
//!
//! The project's page: the title with *Memory settings* and *New record*; *Waiting for approval*, every proposal of
//! the project and its tasks in one queue; then *Records*, the project's own, searched and filtered and grouped by
//! type; at the right the *Project memory* panel, or the inspector of the record a press chose.

mod centered;
mod delete;
mod editor;
mod list;
mod no_match;
mod opened;
mod page;
mod panel;
mod parts;
mod proposal;
mod row;
pub(crate) mod state;
mod toolbar;
pub(crate) mod words;
mod work;

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

use self::state::Made;
pub(crate) use self::{
  centered::Centered,
  list::{ListInput, groups, new_record, row_failure},
  no_match::no_match,
  page::unreadable,
  parts::{group_heading, unbuilt_link},
  toolbar::{FilterBar, FiltersProps},
};
use crate::{project::ShownProject, services::Services};

#[derive(Clone, PartialEq)]
struct Props(ShownProject);

impl DevtoolsInspectable for Props {}

pub fn screen(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<MemoryScreen>(&format!("memory-{}", project.open.project.id()), Props(project.clone()))
}

struct MemoryScreen {
  made: Made,
  retries: lurq::core::Signal<u64>,
}

impl Component for MemoryScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let Props(project) = ctx.props::<Props>().clone();
    Self {
      made: Made::new(ctx, Filters::project(), &project.feed),
      retries: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props(project) = ctx.props::<Props>().clone();
    let state = self.made.state(ctx, &project, None);
    ctx.provide(state.clone());
    let read = read(ctx, &project, self.retries.get());
    page::page(ctx, &state, read, &self.retries)
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
