//! What the window knows about an open project, read once for every tab and screen that shows it (the
//! design doc, "Tabs": nothing is shared between two tabs except the project data): its tasks, its limits
//! and today's usage, its runner's status, how often its runs changed, what automation changed in it,
//! and its merge and removal operations. A [`FeedLoader`] per open project keeps its [`ProjectFeed`]
//! current, mounted by the window rather than a tab, so it follows the project while its tabs are
//! inactive. Only the active project follows every transcript line: an inactive one is read again when
//! a run changes state, not while an agent streams output.
//!
//! The deprecated desktop's `tabs/feed.rs` is the model.

use std::path::{Path, PathBuf};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
  },
  core::Signal,
  node::Element,
};
use orchester_core::{id::ProjectId, time::Timestamp};
use orchester_workspace::{
  background,
  messages::{self, Message},
  runner::{RunnerStatus, Runners},
  tasks::{
    data::ProjectTasks,
    integration_jobs::{IntegrationJobs, ProjectJobState},
    outside::{OutsideChanges, OutsideCounts},
    usage::ProjectUsage,
  },
  workspace::OpenProject,
};

use super::{Projects, hold::ReadHold};
use crate::services::Services;

/// One project's data, shared by every tab and screen that shows it.
#[derive(Clone)]
pub struct ProjectFeed {
  /// The project's tasks as far as they are known; the ones last read while they are read again.
  pub tasks: Signal<TasksState>,
  /// Why the tasks shown could not be read again; they are the ones last read.
  pub stale: Signal<Option<Message>>,
  /// The limits and today's usage; `None` until read.
  pub usage: Signal<Option<ProjectUsage>>,
  /// The project's runner; `None` while it has none in this session.
  pub status: Signal<Option<RunnerStatus>>,
  /// Counts changes of runs and transcripts while a tab of the project is active.
  pub changes: Signal<u64>,
  /// Counts saves of the project's tasks a screen made; the tasks are read again after each
  /// ([`ProjectFeed::saved`]).
  pub revision: Signal<u64>,
  /// Counts the changes automation made to the project through Orchester's MCP server, by kind: the
  /// screens that show what changed read it again in place.
  pub outside: Signal<OutsideCounts>,
  /// The project's merge and worktree-removal operations, which outlive a task page.
  pub jobs: IntegrationJobs,
  /// How those operations stand.
  pub job_state: Signal<ProjectJobState>,
}

impl PartialEq for ProjectFeed {
  fn eq(&self, other: &Self) -> bool {
    self.tasks.id() == other.tasks.id()
  }
}

impl lurq::app::component::DevtoolsInspectable for ProjectFeed {}

impl ProjectFeed {
  pub(super) fn new() -> Self {
    let jobs = IntegrationJobs::default();
    Self {
      tasks: Signal::new(TasksState::Loading),
      stale: Signal::new(None),
      usage: Signal::new(None),
      status: Signal::new(None),
      changes: Signal::new(0),
      revision: Signal::new(0),
      outside: Signal::new(OutsideCounts::default()),
      job_state: Signal::new(jobs.state()),
      jobs,
    }
  }

  /// A screen saved the project's tasks: they are read again, in every tab of the project.
  pub fn saved(&self) {
    self.revision.update(|revision| *revision += 1);
  }
}

/// The project's tasks as far as they are known.
#[derive(Clone, Debug, PartialEq)]
pub enum TasksState {
  /// Not read yet. A reading again after a change keeps the tasks last read instead.
  Loading,
  Ready(ProjectTasks),
  /// The first reading failed, and why.
  Failed(Message),
}

impl lurq::app::component::DevtoolsInspectable for TasksState {}

impl TasksState {
  pub fn ready(&self) -> Option<&ProjectTasks> {
    match self {
      Self::Ready(tasks) => Some(tasks),
      Self::Loading | Self::Failed(_) => None,
    }
  }
}

#[derive(Clone, PartialEq)]
pub struct FeedProps {
  pub open: OpenProject,
  /// Whether the active tab shows the project: only then does its feed follow transcripts.
  pub active: bool,
}

impl lurq::app::component::DevtoolsInspectable for FeedProps {}

/// Keeps one project's feed current. Renders nothing.
pub struct FeedLoader {
  feed: ProjectFeed,
}

impl Component for FeedLoader {
  type Props = FeedProps;

  fn create(ctx: &mut Ctx) -> Self {
    let project = ctx.props::<FeedProps>().open.project.id();
    Self {
      feed: Projects::of(ctx).feed(project),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let FeedProps { open, active } = ctx.props::<FeedProps>().clone();
    let services = Services::of(ctx);
    let runners = services.runners.clone();
    let project = open.project.id();
    let root = open.repo_root;
    let states = counter(ctx, (runners.clone(), root.clone(), Counter::States));
    let changes = counter(ctx, (runners.clone(), root.clone(), Counter::Changes(active)));
    publish(&self.feed.status, status(ctx, &runners, &root));
    publish(&self.feed.changes, changes);
    let outside = outside(ctx, (services.changes.clone(), project));
    publish(&self.feed.outside, outside);
    if let Some(state) = jobs(ctx, self.feed.jobs.clone()) {
      publish(&self.feed.job_state, state);
    }
    // Tasks automation saved count as the screens' own saves.
    let saved = self.feed.revision.get() + outside.tasks;
    let hold = services.hold;
    let (tasks, stale) = tasks(ctx, (root.clone(), saved, states), &hold);
    publish(&self.feed.tasks, tasks);
    publish(&self.feed.stale, stale);
    let day_start = Timestamp::now().local_day_start();
    // Limits automation changed count too: the status bar and New task show them.
    let deps = (root, saved + outside.limits, (states, changes), day_start);
    if let Some(usage) = usage(ctx, deps, &hold) {
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
fn outside(ctx: &mut Ctx, deps: (OutsideChanges, ProjectId)) -> OutsideCounts {
  let count = ctx.stream(
    deps,
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

/// The project's merge and removal operations whenever they change.
fn jobs(ctx: &mut Ctx, jobs: IntegrationJobs) -> Option<ProjectJobState> {
  let state = ctx.stream(jobs, |jobs, emitter: StreamEmitter<ProjectJobState, ()>| async move {
    let mut updates = jobs.subscribe();
    loop {
      let state = updates.borrow_and_update().clone();
      if !emitter.emit(state) || updates.changed().await.is_err() {
        return;
      }
    }
  });
  state.state().get().data
}

/// The project's runner status whenever it changes; the runners' reports of other projects wake it
/// without a render.
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

/// The project's tasks, read again whenever tasks were saved or a run changed state, and why the last
/// reading failed. A failed reading keeps the tasks last read, so no page or form showing them is
/// replaced; only a failed first reading leaves no tasks to show.
fn tasks(ctx: &mut Ctx, deps: (PathBuf, u64, u64), hold: &ReadHold) -> (TasksState, Option<Message>) {
  let hold = hold.clone();
  let load = ctx.future(deps, move |(repo_root, ..)| {
    let hold = hold.clone();
    async move {
      hold.released().await;
      background::run(move || ProjectTasks::load(&repo_root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's tasks");
          messages::store_error(&error)
        })
    }
  });
  let state = load.state().get();
  match (state.data, state.error) {
    (Some(tasks), stale) => (TasksState::Ready(tasks), stale),
    (None, Some(error)) => (TasksState::Failed(error), None),
    (None, None) => (TasksState::Loading, None),
  }
}

/// The project's limits and today's usage, read again whenever settings or runs changed. `None` until
/// read; a failed reading is logged and leaves the last one.
fn usage(ctx: &mut Ctx, deps: (PathBuf, u64, (u64, u64), Timestamp), hold: &ReadHold) -> Option<ProjectUsage> {
  let hold = hold.clone();
  let load = ctx.future(deps, move |(repo_root, _, _, day_start)| {
    let hold = hold.clone();
    async move {
      hold.released().await;
      background::run(move || ProjectUsage::load(&repo_root, day_start))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's limits and usage");
          messages::store_error(&error)
        })
    }
  });
  load.state().get().data
}
