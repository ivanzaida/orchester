//! What the window knows about each project a tab shows, read once per project for every tab of
//! it (design doc, "Tabs": nothing is shared between two tabs except the project data): the
//! project's tasks, its limits and usage, its runner's status and how often its runs changed. A
//! [`FeedLoader`] per project keeps its [`ProjectFeed`] current; it is mounted by the window, not in
//! a tab, so the attention dot and the tab labels follow the project while all its tabs are
//! inactive. Only the active project follows every transcript line: an inactive project is read
//! again when a run changes state, not while an agent streams output.
//!
//! The session's activity log and the runners' work revision are window-wide in the same way
//! ([`WindowFeed`]), so screens in inactive tabs subscribe to signals rather than keep streams of
//! their own.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
  },
  core::Signal,
  node::Element,
};
use orchester_core::{id::ProjectId, time::Timestamp};
use parking_lot::Mutex;

use crate::{
  activity::{Activity, ActivityLog},
  background,
  messages::{self, Message},
  runner::{RunnerStatus, Runners},
  screens::project::TasksState,
  tasks::{
    data::ProjectTasks,
    integration_jobs::{IntegrationJobs, ProjectJobs},
    outside::{OutsideChanges, OutsideCounts},
    usage::ProjectUsage,
  },
  workspace::OpenProject,
};

/// One project's data, shared by every tab of it.
#[derive(Clone)]
pub struct ProjectFeed {
  /// The project's tasks as far as they are known; the previous ones while they are read again.
  pub tasks: Signal<TasksState>,
  /// The limits and today's usage; `None` until read.
  pub usage: Signal<Option<ProjectUsage>>,
  pub connections: Signal<crate::plan_limits::Connections>,
  /// The project's runner; `None` while it has none.
  pub status: Signal<Option<RunnerStatus>>,
  /// Counts run and transcript changes while a tab of the project is active.
  pub changes: Signal<u64>,
  /// Counts saves of the project's tasks; the tasks are read again after each.
  pub revision: Signal<u64>,
  /// Counts the changes automation made to the project through Orchester's MCP server, by kind: the
  /// screens showing what changed read it again in place.
  pub outside: Signal<OutsideCounts>,
  /// Why the tasks shown could not be read again; they are the ones last read.
  pub stale: Signal<Option<Message>>,
  /// Counts imports: screens listing resources are mounted again after one.
  pub resources: Signal<u64>,
  /// Whether the project's task list is shared, and how it stands with its server; `None` while
  /// it is local.
  pub sharing: Signal<Option<crate::sharing::project::state::ProjectSharing>>,
  /// Merge and cleanup operations that outlive a task page and are shared by its tabs.
  pub integration: IntegrationJobs,
  /// Tasks this window's New task form queued in this session: until a run of one is recorded, its
  /// page gives way to the locked form with *Retry run* (02.09F).
  pub queued_by_form: Arc<Mutex<std::collections::HashSet<orchester_core::id::TaskId>>>,
}

/// The feeds of the projects the tabs show, by project, and the projects' merge and removal operations.
#[derive(Clone, Default)]
pub struct ProjectFeeds(Arc<Mutex<HashMap<ProjectId, ProjectFeed>>>, ProjectJobs);

impl PartialEq for ProjectFeeds {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl lurq::app::component::DevtoolsInspectable for ProjectFeeds {}

impl ProjectFeeds {
  /// Feeds whose merge and removal operations are `jobs`, which Orchester's MCP server shares.
  pub fn new(jobs: ProjectJobs) -> Self {
    Self(Arc::default(), jobs)
  }

  /// How the shared projects among those the window read stand with their servers, for the status
  /// bar's server item; reading it follows their changes.
  pub fn sharing(&self) -> Vec<crate::sharing::project::state::ProjectSharing> {
    let feeds: Vec<ProjectFeed> = self.0.lock().values().cloned().collect();
    feeds.iter().filter_map(|feed| feed.sharing.get()).collect()
  }

  /// The feed of `project`, created empty the first time it is asked for.
  pub fn of(&self, project: ProjectId) -> ProjectFeed {
    self
      .0
      .lock()
      .entry(project)
      .or_insert_with(|| ProjectFeed {
        tasks: Signal::new(TasksState::Loading),
        usage: Signal::new(None),
        connections: Signal::new(Default::default()),
        status: Signal::new(None),
        changes: Signal::new(0),
        revision: Signal::new(0),
        outside: Signal::new(OutsideCounts::default()),
        stale: Signal::new(None),
        resources: Signal::new(0),
        sharing: Signal::new(None),
        integration: self.1.of(project),
        queued_by_form: Arc::default(),
      })
      .clone()
  }
}

#[derive(Clone, PartialEq)]
pub struct FeedProps {
  pub open: OpenProject,
  /// Whether the active tab shows the project: only then does it follow transcripts.
  pub active: bool,
}

impl lurq::app::component::DevtoolsInspectable for FeedProps {}

/// Keeps one project's feed current. Renders nothing.
pub struct FeedLoader {
  feed: ProjectFeed,
  /// This loader asked for the shared project's first sync.
  synced: Signal<bool>,
}

impl Component for FeedLoader {
  type Props = FeedProps;

  fn create(ctx: &mut Ctx) -> Self {
    let project = ctx.props::<FeedProps>().open.project.id();
    let feeds = ctx
      .use_context::<ProjectFeeds>()
      .expect("the window provides the project feeds");
    Self {
      feed: feeds.of(project),
      synced: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let FeedProps { open, active } = ctx.props::<FeedProps>().clone();
    let runners = ctx
      .use_context::<Runners>()
      .expect("the root provides Runners before mounting the window");
    let project = open.project.id();
    let root = open.repo_root;
    let states = counter(ctx, (runners.clone(), root.clone(), Counter::States));
    let changes = counter(ctx, (runners.clone(), root.clone(), Counter::Changes(active)));
    publish(&self.feed.status, status(ctx, &runners, &root));
    publish(&self.feed.changes, changes);
    let revision = self.feed.revision.get();
    let resources = self.feed.resources.get();
    let servers = crate::sharing::feed::ServerFeed::of(ctx).servers;
    let shared = super::shared::changes(ctx, &servers);
    publish(
      &self.feed.sharing,
      super::shared::sharing(ctx, (&root, shared), &self.synced),
    );
    publish(&self.feed.connections, connections(ctx, (root.clone(), resources)));
    let outside = outside(ctx, project);
    publish(&self.feed.outside, outside);
    // Tasks automation saved count as the screens' own saves.
    let saved = revision + outside.tasks;
    let (tasks, stale) = tasks(ctx, (root.clone(), saved + shared, states));
    publish(&self.feed.tasks, tasks);
    publish(&self.feed.stale, stale);
    let day_start = Timestamp::now().local_day_start();
    if let Some(usage) = usage(ctx, (root, saved, (states, changes), day_start)) {
      publish(&self.feed.usage, Some(usage));
    }
    Element::new()
  }
}

/// Sets `signal` to `value` unless it holds it already, so its readers render only for a change.
fn publish<T: Clone + PartialEq + Send + Sync + lurq::core::SignalValue + 'static>(signal: &Signal<T>, value: T) {
  if signal.get_untracked() != value {
    signal.set(value);
  }
}

/// The connections reload after a save, check, or resource import in any project tab. A failed
/// read replaces the last one, so the plan limits do not stand for connections no longer known.
fn connections(ctx: &mut Ctx, deps: (PathBuf, u64)) -> crate::plan_limits::Connections {
  use crate::plan_limits::Connections;
  let load = ctx.future(deps, |(root, _)| async move {
    background::run(move || {
      use orchester_core::store::ConnectionStore;
      let db = orchester_store::ProjectDb::open(&root)?;
      db.connections()
        .map(crate::connections::data::agent_connections)
        .map(Connections::Ready)
    })
    .await?
    .map_err(|error| {
      tracing::error!(%error, "cannot read plan limit connections");
      messages::store_error(&error)
    })
  });
  let state = load.state().get();
  match (state.data, state.error) {
    (_, Some(error)) => Connections::Failed(error),
    (Some(connections), None) => connections,
    (None, None) => Connections::Loading,
  }
}

/// Which of a project's change counts a loader follows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Counter {
  States,
  /// Changes of runs and transcripts, followed only while `true`.
  Changes(bool),
}

fn counter(ctx: &mut Ctx, deps: (Runners, PathBuf, Counter)) -> u64 {
  let count = ctx.stream(
    deps,
    |(runners, root, counter), emitter: StreamEmitter<u64, ()>| async move {
      let mut updates = match counter {
        Counter::States => runners.states_of(&root),
        Counter::Changes(true) => runners.changes_of(&root),
        Counter::Changes(false) => return,
      };
      loop {
        let revision = *updates.borrow_and_update();
        if !emitter.emit(revision) || updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  count.state().get().data.unwrap_or(0)
}

/// How often automation changed `project` through Orchester's MCP server. Changes of other projects
/// wake the stream without a render.
fn outside(ctx: &mut Ctx, project: ProjectId) -> OutsideCounts {
  let changes = ctx
    .use_context::<OutsideChanges>()
    .expect("the root provides the outside changes before mounting the window");
  let count = ctx.stream(
    (changes, project),
    |(changes, project), emitter: StreamEmitter<OutsideCounts, ()>| async move {
      let mut updates = changes.subscribe();
      let mut last = None;
      loop {
        let counts = updates.borrow_and_update().get(&project).copied().unwrap_or_default();
        if last != Some(counts) {
          if !emitter.emit(counts) {
            return;
          }
          last = Some(counts);
        }
        if updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  count.state().get().data.unwrap_or_default()
}

/// The project's runner status whenever it changes; the runners' reports of other projects wake
/// it without a render.
fn status(ctx: &mut Ctx, runners: &Runners, root: &Path) -> Option<RunnerStatus> {
  let deps = (runners.clone(), root.to_path_buf());
  let status = ctx.stream(
    deps,
    |(runners, root), emitter: StreamEmitter<Option<RunnerStatus>, ()>| async move {
      let mut work = runners.work_changes();
      let mut last = None;
      loop {
        work.borrow_and_update();
        let status = runners.status_of(&root);
        if last.as_ref() != Some(&status) {
          if !emitter.emit(status.clone()) {
            return;
          }
          last = Some(status);
        }
        if work.changed().await.is_err() {
          return;
        }
      }
    },
  );
  status.state().get().data.flatten()
}

/// The project's tasks, read again whenever tasks were saved or a run changed state, and why the
/// last reading failed. A failed reading keeps the tasks last read on screen, so no page or form
/// showing them is replaced; only a failed first reading leaves no tasks to show.
fn tasks(ctx: &mut Ctx, deps: (PathBuf, u64, u64)) -> (TasksState, Option<Message>) {
  let load = ctx.future(deps, |(repo_root, ..)| async move {
    background::run(move || ProjectTasks::load(&repo_root))
      .await?
      .map_err(|error| {
        tracing::error!(%error, "cannot read the project's tasks");
        messages::store_error(&error)
      })
  });
  let state = load.state().get();
  match (state.data, state.error) {
    (Some(tasks), stale) => (TasksState::Ready(tasks), stale),
    (None, Some(error)) => (TasksState::Failed(error), None),
    (None, None) => (TasksState::Loading, None),
  }
}

/// The project's limits and today's usage, read again whenever settings or runs changed. `None`
/// until read; a failed read is logged and leaves the last one.
fn usage(ctx: &mut Ctx, deps: (PathBuf, u64, (u64, u64), Timestamp)) -> Option<ProjectUsage> {
  let load = ctx.future(deps, |(repo_root, _, _, day_start)| async move {
    background::run(move || ProjectUsage::load(&repo_root, day_start))
      .await?
      .map_err(|error| {
        tracing::error!(%error, "cannot read the project's limits and usage");
        messages::store_error(&error)
      })
  });
  load.state().get().data
}

/// Window-wide data screens in any tab read.
#[derive(Clone)]
pub struct WindowFeed {
  pub limits: Signal<crate::plan_limits::Readings>,
  /// The session's background operations, for the status bar.
  pub activity: Signal<ActivityLog>,
  /// Counts the runners' reports, for All projects.
  pub work: Signal<u64>,
  /// The first unresolved failure of each project, for the tabs' attention dot: it changes only
  /// when a failure comes or is resolved, not with every operation.
  pub failures: Signal<Failures>,
  /// Identity, for comparing props that hold the feed.
  id: Arc<()>,
}

impl PartialEq for WindowFeed {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.id, &other.id)
  }
}

impl lurq::app::component::DevtoolsInspectable for WindowFeed {}

impl WindowFeed {
  pub fn new(activity: &Activity) -> Self {
    Self {
      limits: Signal::new(Default::default()),
      activity: Signal::new(activity.snapshot()),
      work: Signal::new(0),
      failures: Signal::new(Failures::of(&activity.snapshot())),
      id: Arc::default(),
    }
  }

  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the root provides the window feed before mounting screens")
  }
}

/// The first unresolved failure of each project, by the project's name.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Failures(Arc<[(Arc<str>, Message)]>);

impl lurq::app::component::DevtoolsInspectable for Failures {}

impl Failures {
  fn of(log: &ActivityLog) -> Self {
    let mut failures: Vec<(Arc<str>, Message)> = Vec::new();
    for event in log.recent().into_iter().filter(|event| event.is_unresolved()) {
      let Some(project) = event.project() else {
        continue;
      };
      if !failures.iter().any(|(name, _)| &**name == project) {
        failures.push((project.into(), event.title().clone()));
      }
    }
    Self(failures.into())
  }

  /// The unresolved failure of the project named `name`, if it has one.
  pub fn of_project(&self, name: &str) -> Option<&Message> {
    self
      .0
      .iter()
      .find(|(project, _)| &**project == name)
      .map(|(_, failure)| failure)
  }
}

/// Keeps the [`WindowFeed`] current; mounted by the root, which is never offstage. Renders
/// nothing.
pub struct WindowFeedLoader;

impl Component for WindowFeedLoader {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let feed = WindowFeed::of(ctx);
    let activity = Activity::of(ctx);
    let log = ctx.stream(
      activity,
      |activity, emitter: StreamEmitter<ActivityLog, ()>| async move {
        let mut updates = activity.subscribe();
        loop {
          let log = updates.borrow_and_update().clone();
          if !emitter.emit(log) || updates.changed().await.is_err() {
            return;
          }
        }
      },
    );
    if let Some(log) = log.state().get().data {
      publish(&feed.failures, Failures::of(&log));
      publish(&feed.activity, log);
    }
    if let Some(runners) = ctx.use_context::<Runners>() {
      let limits = ctx.stream(
        runners.clone(),
        |runners, emitter: StreamEmitter<crate::plan_limits::Readings, ()>| async move {
          let mut updates = runners.plan_limits();
          loop {
            let readings = updates.borrow_and_update().clone();
            if !emitter.emit(crate::plan_limits::Readings(readings)) || updates.changed().await.is_err() {
              return;
            }
          }
        },
      );
      if let Some(readings) = limits.state().get().data {
        publish(&feed.limits, readings);
      }
      let work = ctx.stream(runners, |runners, emitter: StreamEmitter<u64, ()>| async move {
        let mut updates = runners.work_changes();
        loop {
          let revision = *updates.borrow_and_update();
          if !emitter.emit(revision) || updates.changed().await.is_err() {
            return;
          }
        }
      });
      publish(&feed.work, work.state().get().data.unwrap_or(0));
    }
    Element::new()
  }
}
