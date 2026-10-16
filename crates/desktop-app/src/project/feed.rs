//! What the window knows about an open project, read once for every tab and screen that shows it (the
//! design doc, "Tabs": nothing is shared between two tabs except the project data): its tasks, its limits
//! and today's usage, its runner's status, how often its runs changed, what automation changed in it,
//! and its merge and removal operations. A [`FeedLoader`] per open project keeps its [`ProjectFeed`]
//! current, mounted by the window rather than a tab, so it follows the project while its tabs are
//! inactive. Only the active project follows every transcript line: an inactive one is read again when
//! a run changes state, not while an agent streams output.
//!
//! The deprecated desktop's `tabs/feed.rs` is the model.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
  },
  core::Signal,
  node::Element,
};
use orchester_core::{id::ProjectId, run::TaskPhase, time::Timestamp};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  background,
  messages::{self, Message},
  runner::{RunnerStatus, Runners, state::RunnerState},
  tasks::{
    data::ProjectTasks,
    integration_jobs::{IntegrationJobs, ProjectJobState},
    outside::{OutsideChanges, OutsideCounts},
    usage::ProjectUsage,
  },
  workspace::OpenProject,
};
use parking_lot::Mutex;

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
  /// Counts changes of runs and transcripts while a tab of the project is active, at most once per
  /// [`TRANSCRIPT_PACE`].
  pub changes: Signal<u64>,
  /// Counts changes of the state of the project's runs: a run started, waits or ended. A transcript's lines do
  /// not count.
  pub states: Signal<u64>,
  /// Whether one of the project's runs runs, as its tasks were last read.
  pub running: Signal<bool>,
  /// Counts saves of the project's tasks a screen made; the tasks are read again after each
  /// ([`ProjectFeed::saved`]).
  pub revision: Signal<u64>,
  /// Counts the changes automation made to the project through Orchester's MCP server, by kind: the
  /// screens that show what changed read it again in place.
  pub outside: Signal<OutsideCounts>,
  /// The project's merge and worktree-removal operations, which outlive a task page: its part of the
  /// session's registry (`Services::jobs`), so what a page or automation started shows here.
  pub jobs: IntegrationJobs,
  /// How those operations stand.
  pub job_state: Signal<ProjectJobState>,
  /// The task forms of the project that hold edits not saved, which their tabs mark.
  pub unsaved: Signal<UnsavedForms>,
  /// Saved pipeline names read by editor tabs, shared with their chrome labels.
  pub pipeline_names: Signal<PipelineNames>,
  /// The tasks last read while no runner served the project, as they stand without one.
  offline: Arc<Mutex<Option<Offline>>>,
}

/// A reading of the tasks and the same tasks without a runner, made from it once: every caller then shares
/// what waits in it, worked out once (`WaitingOnce`).
struct Offline {
  /// Kept so no later reading can take its place in memory and pass for it.
  read: ProjectTasks,
  without: ProjectTasks,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PipelineNames(pub std::sync::Arc<std::collections::BTreeMap<orchester_core::id::ResourceId, String>>);

impl lurq::app::component::DevtoolsInspectable for PipelineNames {}

/// Task forms holding edits that are not saved: Edit task by its task's key, New task as `None`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnsavedForms(pub std::sync::Arc<std::collections::BTreeSet<Option<String>>>);

impl lurq::app::component::DevtoolsInspectable for UnsavedForms {}

impl UnsavedForms {
  pub fn contains(&self, task: Option<&str>) -> bool {
    self.0.contains(&task.map(str::to_owned))
  }
}

impl ProjectFeed {
  /// Records a saved pipeline name without notifying chrome when it is unchanged.
  pub fn pipeline_named(&self, id: orchester_core::id::ResourceId, name: &str) {
    if self.pipeline_names.get_untracked().0.get(&id).map(String::as_str) != Some(name) {
      self.pipeline_names.update(|names| {
        std::sync::Arc::make_mut(&mut names.0).insert(id, name.to_owned());
      });
    }
  }

  /// Marks the form of `task` (`None`: New task) as holding unsaved edits, or not.
  pub fn mark_unsaved(&self, task: Option<&str>, unsaved: bool) {
    let form = task.map(str::to_owned);
    if self.unsaved.get_untracked().0.contains(&form) == unsaved {
      return;
    }
    self.unsaved.update(|forms| {
      let set = std::sync::Arc::make_mut(&mut forms.0);
      if unsaved {
        set.insert(form);
      } else {
        set.remove(&form);
      }
    });
  }
}

impl PartialEq for ProjectFeed {
  fn eq(&self, other: &Self) -> bool {
    self.tasks.id() == other.tasks.id()
  }
}

impl lurq::app::component::DevtoolsInspectable for ProjectFeed {}

impl ProjectFeed {
  /// An empty feed that follows `jobs`, the project's operations in the session's registry.
  pub(super) fn new(jobs: IntegrationJobs) -> Self {
    Self {
      tasks: Signal::new(TasksState::Loading),
      stale: Signal::new(None),
      usage: Signal::new(None),
      status: Signal::new(None),
      changes: Signal::new(0),
      states: Signal::new(0),
      running: Signal::new(false),
      revision: Signal::new(0),
      outside: Signal::new(OutsideCounts::default()),
      job_state: Signal::new(jobs.state()),
      unsaved: Signal::new(UnsavedForms::default()),
      pipeline_names: Signal::new(PipelineNames::default()),
      offline: Arc::default(),
      jobs,
    }
  }

  /// A screen saved the project's tasks: they are read again, in every tab of the project.
  pub fn saved(&self) {
    self.revision.update(|revision| *revision += 1);
  }

  /// The count of the project's run changes a view of runs follows: while one of its runs runs (`live`)
  /// every change, its transcript's lines too; otherwise a run's change of state only, as nothing else of
  /// the runs it shows changes. Reading it in a render re-renders it when that count moves.
  pub fn run_changes(&self, live: bool) -> u64 {
    if live { self.changes.get() } else { self.states.get() }
  }

  /// Whether a runner serves the project at `repo_root` now. Reading it in a render re-renders it when
  /// that changes.
  pub fn runner_online(&self, repo_root: &Path) -> bool {
    self
      .status
      .with(|status| status.as_ref().map(|status| status.state_for(repo_root)) == Some(RunnerState::Online))
  }

  /// The tasks of the project at `repo_root` as they stand with its runner: while none serves the
  /// project, a run recorded as running has lost its agent and is interrupted
  /// (`ProjectTasks::without_runner`). Reading it in a render re-renders it when either changes.
  pub fn tasks_with_runner(&self, repo_root: &Path) -> TasksState {
    let online = self.runner_online(repo_root);
    match self.tasks.get() {
      TasksState::Ready(tasks) if !online => TasksState::Ready(self.without_runner(tasks)),
      state => state,
    }
  }

  /// `tasks` without a runner, made once per reading.
  fn without_runner(&self, tasks: ProjectTasks) -> ProjectTasks {
    let mut offline = self.offline.lock();
    // Every clone of a reading shares its fields; another reading has its own.
    if let Some(made) = offline
      .as_ref()
      .filter(|made| Arc::ptr_eq(&made.read.phases, &tasks.phases))
    {
      return made.without.clone();
    }
    let without = tasks.without_runner();
    *offline = Some(Offline {
      read: tasks,
      without: without.clone(),
    });
    without
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
    publish(&self.feed.states, states);
    let outside = outside(ctx, (services.changes.clone(), project));
    publish(&self.feed.outside, outside);
    if let Some(state) = jobs(ctx, self.feed.jobs.clone()) {
      publish(&self.feed.job_state, state);
    }
    // Tasks automation saved count as the screens' own saves.
    let saved = self.feed.revision.get() + outside.tasks;
    let hold = services.hold;
    let (tasks, stale) = tasks(ctx, (root.clone(), saved, states), (&hold, &services.programs));
    let running = tasks.ready().is_some_and(runs_now);
    publish(&self.feed.running, running);
    publish(&self.feed.tasks, tasks);
    if let Some(stale) = stale {
      publish(&self.feed.stale, stale);
    }
    let day_start = Timestamp::now().local_day_start();
    // Limits automation changed count too: the status bar and New task show them. Today's use grows with each
    // agent turn, which the runner reports as transcript activity: it is read again on those while a run runs
    // and the project is followed that closely, on run state changes otherwise.
    let runs = if running && active { changes } else { states };
    let deps = (root, saved + outside.limits, runs, day_start);
    if let Some(usage) = usage(ctx, deps, &hold) {
      publish(&self.feed.usage, Some(usage));
    }
    Element::new()
  }
}

/// Whether one of `tasks`' latest runs runs.
fn runs_now(tasks: &ProjectTasks) -> bool {
  tasks
    .phases
    .values()
    .any(|latest| latest.phase() == Some(TaskPhase::Running))
}

/// Sets `signal` to `value` unless it holds it already, so its readers render only for a change.
fn publish<T: Clone + PartialEq + Send + Sync + lurq::core::SignalValue + 'static>(signal: &Signal<T>, value: T) {
  if signal.get_untracked() != value {
    signal.set(value);
  }
}

/// How long the count of a project's run changes stays put after it moved: an agent writing lines faster
/// than that reads what shows its transcript again once per pace, with all the lines since.
pub const TRANSCRIPT_PACE: Duration = Duration::from_millis(100);

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
        if !emitter.emit(revision) {
          return;
        }
        if matches!(counter, Counter::Changes(_)) {
          tokio::time::sleep(TRANSCRIPT_PACE).await;
        }
        if updates.changed().await.is_err() {
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
/// reading failed: `None` while a reading of tasks already shown is under way, which keeps what the
/// previous reading said until this one ends. A failed reading keeps the tasks last read, so no page or
/// form showing them is replaced; only a failed first reading leaves no tasks to show.
fn tasks(
  ctx: &mut Ctx,
  deps: (PathBuf, u64, u64),
  (hold, programs): (&ReadHold, &Programs),
) -> (TasksState, Option<Option<Message>>) {
  let (hold, programs) = (hold.clone(), programs.clone());
  let load = ctx.future(deps, move |(repo_root, ..)| {
    tracing::trace!(target: crate::app_log::READ_LOG, read = "project tasks");
    let (hold, programs) = (hold.clone(), programs.clone());
    async move {
      hold.released().await;
      background::run(move || ProjectTasks::load(&repo_root, &programs))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's tasks");
          messages::store_error(&error)
        })
    }
  });
  let state = load.state().get();
  let reading = state.is_pending();
  match (state.data, state.error) {
    (Some(tasks), _) if reading => (TasksState::Ready(tasks), None),
    (Some(tasks), stale) => (TasksState::Ready(tasks), Some(stale)),
    (None, Some(error)) => (TasksState::Failed(error), Some(None)),
    (None, None) => (TasksState::Loading, Some(None)),
  }
}

/// The project's limits and today's usage, read again whenever settings or runs changed. `None` until
/// read; a failed reading is logged and leaves the last one.
fn usage(ctx: &mut Ctx, deps: (PathBuf, u64, u64, Timestamp), hold: &ReadHold) -> Option<ProjectUsage> {
  let hold = hold.clone();
  let load = ctx.future(deps, move |(repo_root, _, _, day_start)| {
    tracing::trace!(target: crate::app_log::READ_LOG, read = "project usage");
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
