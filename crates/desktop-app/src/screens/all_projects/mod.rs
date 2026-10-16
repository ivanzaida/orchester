//! All projects (`All projects · dark` `S5HDKf`, its states `kwpO2`) and the welcome of a first launch
//! (`Welcome · first launch · dark` `UQZRa`): the projects the application database knows, each with where it
//! stands, its runs and when it was last opened, read without starting a runner; or, while a session has no
//! project yet, the welcome. The list and its folders show at once, the counts once each project's data is
//! read (a dash until then), and the runners' states as they report. Selecting a project that opens shows its
//! Overview; *Open project folder…* opens the project a folder keeps ([`folder`]); *Add project* opens Add
//! project; the × asks in place whether to remove a project from the list ([`remove`]). The rules are
//! `orchester_workspace::project_list`'s.

mod folder;
mod page;
mod remove;
mod row;
mod welcome;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

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
  workspace::{ListedData, ProjectEntry},
};
use parking_lot::Mutex;

use crate::{routes::Screen, services::Services};

/// The element of the screen, as every screen has one (`screen-<name>`).
fn screen_id() -> String {
  format!("screen-{}", Screen::AllProjects.name())
}

pub fn screen(ctx: &mut Ctx) -> Element {
  ctx.mount_keyed::<AllProjects>("all-projects", ())
}

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
  /// The projects' data as last read, and the runners' reports it was read at.
  read: Arc<Mutex<(Reported, ListedData)>>,
}

impl Component for AllProjects {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      ask: Ask {
        revision: ctx.signal(0),
        removing: ctx.signal(Removing::default()),
      },
      followed: Mutex::new(false),
      read: Arc::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // First in every render: lurq keeps a component's futures by the order it asks for them.
    let services = Services::of(ctx);
    let open = folder::open_action(ctx, &services);
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
    let reports = reports(ctx, &services.runners, &entries);
    let data = read_data(ctx, (&services, &self.read), Entries(entries.clone(), reports));
    folder::follow(ctx, &open, &self.followed);
    let failed = listing.error.clone().filter(|_| !listing.is_pending());
    match (listing.data, failed) {
      (_, Some(error)) => page::page(ctx, &open, page::Body::Failed(&error)),
      (Some(Listing::Welcome), None) => welcome::welcome(ctx, &open),
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

/// What each listed project's database says, read after the list is shown and again whenever its runner
/// reports; the previous data stays on screen while it is read again. Held while QA holds reads
/// (`hold_reads`).
fn read_data(
  ctx: &mut Ctx,
  (services, read): (&Services, &Arc<Mutex<(Reported, ListedData)>>),
  entries: Entries,
) -> ListedData {
  let (hold, programs, read) = (services.hold.clone(), services.programs.clone(), Arc::clone(read));
  ctx
    .future(entries, move |Entries(entries, reported)| {
      let (read_at, before) = read.lock().clone();
      // A project whose runner reported since, or that was not read yet.
      let again: Vec<ProjectId> = entries
        .iter()
        .filter(|entry| !entry.folder_missing)
        .filter(|entry| before.of(entry.project.id).is_none() || read_at.of(entry) != reported.of(entry))
        .map(|entry| entry.project.id)
        .collect();
      for _ in &again {
        tracing::trace!(target: crate::app_log::READ_LOG, read = "listed project");
      }
      let (hold, programs, read) = (hold.clone(), programs.clone(), Arc::clone(&read));
      async move {
        hold.released().await;
        let data = background::run(move || before.read_again(&entries, &again, &programs)).await?;
        *read.lock() = (reported, data.clone());
        Ok::<_, messages::Message>(data)
      }
    })
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// The entries a reading of their data is for, and the runners' reports it follows.
#[derive(Clone, PartialEq)]
struct Entries(Vec<ProjectEntry>, Reported);

impl DevtoolsInspectable for Entries {}

/// How often each listed project's runner reported a change of its status, by the project's folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Reported(Arc<HashMap<PathBuf, u64>>);

impl DevtoolsInspectable for Reported {}

impl Reported {
  fn of(&self, entry: &ProjectEntry) -> u64 {
    self.0.get(&entry.project.repo_root).copied().unwrap_or(0)
  }
}

/// The runners' reports about the projects of `entries`: renders the screen again whenever a listed project's
/// runner status changed, for its row and to read its data again.
fn reports(ctx: &mut Ctx, runners: &Runners, entries: &[ProjectEntry]) -> Reported {
  let roots: Vec<PathBuf> = entries.iter().map(|entry| entry.project.repo_root.clone()).collect();
  let reports = ctx.stream(
    (runners.clone(), roots),
    |(runners, roots), emitter: StreamEmitter<Reported, ()>| async move {
      let mut updates = runners.work_changes();
      let mut statuses = HashMap::new();
      let mut counts = HashMap::new();
      let mut first = true;
      loop {
        updates.borrow_and_update();
        let mut reported = first;
        for root in &roots {
          let status = runners.status_of(root);
          // The first status seen of a project is no report about it.
          if statuses.get(root) != Some(&status) && statuses.insert(root.clone(), status).is_some() {
            *counts.entry(root.clone()).or_insert(0) += 1;
            reported = true;
          }
        }
        first = false;
        // Another project's report wakes the stream without a render.
        if reported && !emitter.emit(Reported(Arc::new(counts.clone()))) {
          return;
        }
        if updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  reports.state().get().data.unwrap_or_default()
}
