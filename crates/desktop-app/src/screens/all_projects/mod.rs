//! All projects (`All projects · dark` `S5HDKf`, its states `kwpO2`; the returning start, 09 `i5G19A` with a team and
//! 09b `dLOCJ` without) and the welcome of a first launch (`Welcome · first launch · dark` `UQZRa`): the projects the
//! application database knows, each with where it is, where it stands, its runs and when it was last opened, read
//! without starting a runner; or, while a session has no project yet, the welcome. The list and its folders show at
//! once, the counts and where each project is once its data is read (a dash until then), and the runners' states as
//! they report. Selecting a project that opens shows its Overview; *Open project folder…* opens the project a folder
//! keeps ([`folder`]); *Add project* opens Add project; *Join a team* and *Open shared project…* open the teams in
//! Settings › Orchester Cloud; a joined team's project with no checkout here is offered above the list until *Not
//! now* ([`teams`], [`offer`]); an agent CLI that is not found is warned of above the list and on the welcome
//! ([`agent_cli`]); the × asks in place whether to remove a project from the list ([`remove`]); a row under the pointer
//! offers to move its project between this computer and the account ([`move_project`]); and while nobody is signed in a
//! line says what waits for a sign-in ([`waiting`]). The *Usage* tab shows every project's usage (`rHA8G`,
//! `crate::screens::usage::everywhere`). The rules are `orchester_workspace::project_list`'s and
//! `orchester_workspace::sharing`'s.

mod agent_cli;
mod folder;
mod move_project;
mod offer;
mod page;
mod remove;
mod row;
mod teams;
mod waiting;
mod welcome;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureState, StreamEmitter},
  },
  core::Signal,
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background, messages,
  project_list::{self, Listing},
  runner::{RunnerStatus, Runners},
  sharing::cloud::SignIn,
  usage::UsageQuery,
  workspace::{ListedData, ProjectEntry, Workspace},
};
use parking_lot::Mutex;

use self::{agent_cli::AgentCli, move_project::Moving, teams::Teams};
use crate::{root::follow_sign_in, routes::Screen, screens::settings::orchester::InvitesNotice, services::Services};

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

/// The screen's own state: what was removed or moved, and which project's removal or move is asked.
#[derive(Clone)]
struct Ask {
  /// Counts removals and moves, so the list is read again after one.
  revision: Signal<u64>,
  /// The project whose removal is asked under its row.
  removing: Signal<Removing>,
  /// The project whose move is asked under its row.
  moving: Signal<Moving>,
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
  /// What the runners reported of the listed projects so far, kept across the list's changes.
  seen: Arc<Mutex<Seen>>,
  /// The answers of the invites notice, which outlive the notice.
  invites: InvitesNotice,
  /// What was put off of the teams' projects offered.
  teams: Teams,
  /// The warning that an agent CLI is not found.
  cli: AgentCli,
}

impl Component for AllProjects {
  type Props = Tab;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      ask: Ask {
        revision: ctx.signal(0),
        removing: ctx.signal(Removing::default()),
        moving: ctx.signal(Moving::default()),
      },
      followed: Mutex::new(false),
      read: Arc::default(),
      seen: Arc::default(),
      invites: InvitesNotice::new(ctx),
      teams: Teams::new(ctx),
      cli: AgentCli::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // First in every render: lurq keeps a component's futures by the order it asks for them.
    let services = Services::of(ctx);
    let sign_in = follow_sign_in(ctx, &services.cloud);
    let open = folder::open_action(ctx, &services);
    let revision = self.ask.revision.get();
    let listing = read_listing(ctx, &services.workspace, revision, &sign_in);
    let waiting = waiting::read(ctx, revision, &sign_in, &services.workspace);
    let entries = match &listing.data {
      Some(Listing::Projects(entries)) => entries.clone(),
      Some(Listing::Welcome) | None => Vec::new(),
    };
    let failed = listing.error.clone().filter(|_| !listing.is_pending());
    let cli = self.cli.read(
      ctx,
      &services,
      agent_cli::listed(listing.data.as_ref(), failed.is_some()),
    );
    // The teams are read once the page shows: for the listed projects, or for the header alone when the list failed.
    let shown = match (&listing.data, &failed) {
      (Some(Listing::Projects(entries)), _) => Some(entries.as_slice()),
      (_, Some(_)) => Some(&[][..]),
      (Some(Listing::Welcome) | None, None) => None,
    };
    let reports = reports(ctx, (&services.runners, &self.seen), &entries);
    let work = reports.total();
    let data = read_data(ctx, (&services, &self.read), Entries(entries.clone(), reports));
    folder::follow(ctx, &open, &self.followed);
    let invites = InvitesNotice::read(ctx);
    let teams = self.teams.read(ctx, &services.servers, shown);
    let heading = page::Heading {
      open: &open,
      joined: teams.joined.as_ref().map(|joined| joined.servers.as_slice()),
    };
    let tab = ctx.props::<Tab>().clone();
    match (listing.data, failed) {
      (_, Some(error)) => {
        let above = page::Above {
          notice: self.invites.notice(ctx, &invites),
          ..page::Above::default()
        };
        page::page(ctx, heading, page::Body::Failed(&error), above)
      }
      (Some(Listing::Welcome), None) => {
        let cli = self.cli.element(ctx, &cli);
        welcome::welcome(ctx, &open, cli)
      }
      (Some(Listing::Projects(_)), None) if let Tab::Usage(query) = tab => {
        let usage = crate::screens::usage::everywhere(ctx, &query, work);
        page::page(ctx, heading, page::Body::Usage(usage), page::Above::default())
      }
      (Some(Listing::Projects(_)), None) => {
        let rows = project_list::listed(&entries, &data, &services.runners);
        let line = waiting::line(ctx, waiting, &sign_in);
        let body = if !rows.is_empty() {
          page::Body::List {
            rows: &rows,
            places: teams.joined.as_ref().map(|joined| &joined.places),
            signed_in: sign_in.is_signed_in(),
            ask: &self.ask,
          }
        } else if line.is_some() {
          page::Body::NothingHere
        } else {
          page::Body::Empty
        };
        let above = page::Above {
          waiting: line,
          cli: self.cli.element(ctx, &cli),
          notice: self.invites.notice(ctx, &invites),
          offer: teams
            .offer
            .as_ref()
            .map(|offered| offer::strip(ctx, offered, &teams.answer)),
        };
        page::page(ctx, heading, body, above)
      }
      // Reading the list takes a moment; the screen stays empty meanwhile.
      (None, None) => lurq::components::Column::new().id(screen_id()).into(),
    }
  }
}

/// What the screen lists, read when it shows, after a removal or a move (`revision`) and whenever the sign-in changes.
fn read_listing(
  ctx: &mut Ctx,
  workspace: &Workspace,
  revision: u64,
  sign_in: &SignIn,
) -> FutureState<Listing, messages::Message> {
  ctx
    .future(
      (workspace.clone(), revision, sign_in.account()),
      |(workspace, ..)| async move {
        background::run(move || workspace.listing()).await?.map_err(|error| {
          tracing::error!(%error, "cannot read the workspace's projects");
          messages::store_error(&error)
        })
      },
    )
    .state()
    .get()
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

  /// How often the listed projects' runners reported in all; it grows with each report, as no count goes back.
  fn total(&self) -> u64 {
    self.0.values().sum()
  }
}

/// The runner status last seen of each listed project, and how often it changed since the screen showed.
#[derive(Default)]
struct Seen {
  statuses: HashMap<PathBuf, Option<RunnerStatus>>,
  counts: HashMap<PathBuf, u64>,
}

impl Seen {
  /// Takes the statuses of `roots` now; whether one changed since it was last seen. The first status seen of a
  /// project is no report about it.
  fn look(&mut self, runners: &Runners, roots: &[PathBuf]) -> bool {
    let mut reported = false;
    for root in roots {
      let status = runners.status_of(root);
      if self.statuses.get(root) != Some(&status) && self.statuses.insert(root.clone(), status).is_some() {
        *self.counts.entry(root.clone()).or_insert(0) += 1;
        reported = true;
      }
    }
    reported
  }
}

/// The runners' reports about the projects of `entries`: renders the screen again whenever a listed project's
/// runner status changed, for its row and to read its data again. What was seen outlives the stream, which starts
/// again when the list changes: a report that came meanwhile still counts, and no count goes back.
fn reports(ctx: &mut Ctx, (runners, seen): (&Runners, &Arc<Mutex<Seen>>), entries: &[ProjectEntry]) -> Reported {
  let roots: Vec<PathBuf> = entries.iter().map(|entry| entry.project.repo_root.clone()).collect();
  let seen = Arc::clone(seen);
  let reports = ctx.stream(
    (runners.clone(), roots),
    move |(runners, roots), emitter: StreamEmitter<Reported, ()>| {
      let seen = Arc::clone(&seen);
      async move {
        let mut updates = runners.work_changes();
        let mut first = true;
        loop {
          updates.borrow_and_update();
          let counts = {
            let mut seen = seen.lock();
            (seen.look(&runners, &roots) || first).then(|| seen.counts.clone())
          };
          first = false;
          // Another project's report wakes the stream without a render.
          if let Some(counts) = counts
            && !emitter.emit(Reported(Arc::new(counts)))
          {
            return;
          }
          if updates.changed().await.is_err() {
            return;
          }
        }
      }
    },
  );
  reports.state().get().data.unwrap_or_default()
}
