//! All projects (`All projects · dark` `S5HDKf`, its states `kwpO2`) and the welcome of a first launch
//! (`Welcome · first launch · dark` `UQZRa`): the projects the application database knows, each with where it
//! stands, its runs and when it was last opened, read without starting a runner; or, while a session has no
//! project yet, the welcome. The list and its folders show at once, the counts once each project's data is
//! read (a dash until then), and the runners' states as they report. Selecting a project that opens shows its
//! Overview; *Open project folder…* opens the project a folder keeps ([`folder`]); *Add project* opens Add
//! project; the × asks in place whether to remove a project from the list ([`remove`]). The *Usage* tab
//! shows every project's usage (`rHA8G`, `crate::screens::usage::everywhere`). The rules are
//! `orchester_workspace::project_list`'s.

mod folder;
mod page;
mod remove;
mod row;
mod welcome;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
  },
  core::Signal,
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background, messages,
  project_list::{self, Listing},
  runner::Runners,
  usage::UsageQuery,
  workspace::{ListedData, ProjectEntry},
};
use parking_lot::Mutex;

use crate::{routes::Screen, services::Services};

/// The element of the screen, as every screen has one (`screen-<name>`).
fn screen_id() -> String {
  format!("screen-{}", Screen::AllProjects.name())
}

pub fn screen(ctx: &mut Ctx) -> Element {
  ctx.mount_keyed::<AllProjects>("all-projects", Tab::Projects)
}

/// The *Usage* tab, showing `query`.
pub fn usage_screen(ctx: &mut Ctx, query: UsageQuery) -> Element {
  ctx.mount_keyed::<AllProjects>("all-projects", Tab::Usage(Box::new(query)))
}

/// The tab the page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Tab {
  Projects,
  Usage(Box<UsageQuery>),
}

impl DevtoolsInspectable for Tab {}

/// The screen's own state: what was removed, and which project's removal is asked.
#[derive(Clone)]
struct Ask {
  /// Counts removals, so the list is read again after one.
  revision: Signal<u64>,
  /// The project whose removal is asked under its row.
  removing: Signal<Removing>,
}

/// The project whose removal is asked, if any.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Removing(Option<ProjectId>);

impl DevtoolsInspectable for Removing {}

struct AllProjects {
  ask: Ask,
  /// Whether *Open project folder…*'s end is followed already; once per screen.
  followed: Mutex<bool>,
}

impl Component for AllProjects {
  type Props = Tab;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      ask: Ask {
        revision: ctx.signal(0),
        removing: ctx.signal(Removing::default()),
      },
      followed: Mutex::new(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // First in every render: lurq keeps a component's futures by the order it asks for them.
    let services = Services::of(ctx);
    let open = folder::open_action(ctx, &services);
    let work = work_changes(ctx, &services.runners);
    let workspace = services.workspace.clone();
    let listing = ctx
      .future((workspace, self.ask.revision.get()), |(workspace, _)| async move {
        background::run(move || workspace.listing()).await?.map_err(|error| {
          tracing::error!(%error, "cannot read the workspace's projects");
          messages::store_error(&error)
        })
      })
      .state()
      .get();
    let entries = match &listing.data {
      Some(Listing::Projects(entries)) => entries.clone(),
      Some(Listing::Welcome) | None => Vec::new(),
    };
    let data = read_data(ctx, &services, Entries(entries.clone(), work));
    folder::follow(ctx, &open, &self.followed);
    let failed = listing.error.clone().filter(|_| !listing.is_pending());
    let tab = ctx.props::<Tab>().clone();
    match (listing.data, failed) {
      (_, Some(error)) => page::page(ctx, &open, page::Body::Failed(&error)),
      (Some(Listing::Welcome), None) => welcome::welcome(ctx, &open),
      (Some(Listing::Projects(_)), None) if let Tab::Usage(query) = tab => {
        let usage = crate::screens::usage::everywhere(ctx, &query, work);
        page::page(ctx, &open, page::Body::Usage(usage))
      }
      (Some(Listing::Projects(_)), None) => {
        let rows = project_list::listed(&entries, &data, &services.runners);
        let body = if rows.is_empty() {
          page::Body::Empty
        } else {
          page::Body::List {
            rows: &rows,
            ask: &self.ask,
          }
        };
        page::page(ctx, &open, body)
      }
      // Reading the list takes a moment; the screen stays empty meanwhile.
      (None, None) => lurq::components::Column::new().id(screen_id()).into(),
    }
  }
}

/// What each listed project's database says, read after the list is shown and again whenever a runner
/// reports; the previous data stays on screen while it is read again. Held while QA holds reads
/// (`hold_reads`).
fn read_data(ctx: &mut Ctx, services: &Services, entries: Entries) -> ListedData {
  let (hold, programs) = (services.hold.clone(), services.programs.clone());
  ctx
    .future(entries, move |Entries(entries, _)| {
      let (hold, programs) = (hold.clone(), programs.clone());
      async move {
        hold.released().await;
        background::run(move || ListedData::read(&entries, &programs)).await
      }
    })
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// The entries a reading of their data is for, and the runners' work revision it follows.
#[derive(Clone, PartialEq)]
struct Entries(Vec<ProjectEntry>, u64);

impl DevtoolsInspectable for Entries {}

/// The runners' work revision; reading it renders the screen again whenever a runner reports.
fn work_changes(ctx: &mut Ctx, runners: &Runners) -> u64 {
  let changes = ctx.stream(runners.clone(), |runners, emitter: StreamEmitter<u64, ()>| async move {
    let mut updates = runners.work_changes();
    loop {
      let revision = *updates.borrow_and_update();
      if !emitter.emit(revision) || updates.changed().await.is_err() {
        return;
      }
    }
  });
  changes.state().get().data.unwrap_or(0)
}
