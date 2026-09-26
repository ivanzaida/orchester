//! Project shell: the sidebar (`zo9Py`) beside the open project screen — the overview
//! (`Twtqc`), the task list (`J9467`), the new task form (`l2YA4`), a task (`RD4pc`, `SfXU1`),
//! pull requests (`wGtxT`) and one pull request
//! (`Rrmhf`), agent profiles (`WPmtS`),
//! connections (`a4Lqb`) or Project settings (`QV1af`), above the status bar (`pmOpt`). The shell reads the project's
//! tasks once and hands them to the sidebar, the screen and the status bar.

mod agents;
mod connections;
mod new_task;
mod overview;
mod page;
mod pipelines;
mod pull_requests;
mod runner;
mod settings;
mod sidebar;
mod task;
mod task_rows;
mod tasks;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, StreamEmitter},
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::{ResourceId, TaskId};

use self::{
  agents::AgentsScreen,
  connections::ConnectionsScreen,
  new_task::{NewTaskForm, NewTaskProps},
  pipelines::{PipelinesProps, PipelinesScreen},
  pull_requests::{PullRequestProps, PullRequestScreen, PullRequestsScreen},
  settings::SettingsScreen,
  sidebar::sidebar,
  task::{TaskProps, TaskScreen},
  tasks::{TasksProps, TasksScreen},
};
use super::status_bar::{ProjectBar, StatusBar, StatusBarProps};
use crate::{
  background,
  messages::{self, Message},
  runner::{RunnerStatus, Runners},
  tasks::{data::ProjectTasks, list},
  workspace::OpenProject,
};

/// The screen shown beside the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
  Overview,
  Tasks,
  NewTask,
  /// One task and its latest run (`RD4pc`, `SfXU1`).
  Task(TaskId),
  PullRequests,
  /// One pull request of the linked repository, by number (`Rrmhf`).
  PullRequest(u64),
  Pipelines,
  Agents,
  Connections,
  Settings,
}

impl lurq::app::component::DevtoolsInspectable for Section {}

/// The profile the Agents screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentSelection {
  /// The first profile, if there is one.
  First,
  Profile(ResourceId),
  /// A profile being written that is not saved yet.
  New,
}

impl lurq::app::component::DevtoolsInspectable for AgentSelection {}

/// A pipeline to open when the Pipelines screen next mounts. `revision` changes with every
/// request, so a request remounts the screen even while it is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PipelineRequest {
  revision: u64,
  id: Option<ResourceId>,
}

impl lurq::app::component::DevtoolsInspectable for PipelineRequest {}

/// Provided by the shell: switching screens and announcing that tasks changed.
#[derive(Clone)]
pub struct ProjectNav {
  section: Signal<Section>,
  revision: Signal<u64>,
  agent: Signal<AgentSelection>,
  pipeline: Signal<PipelineRequest>,
  /// Connections opens Add GitHub connection when it next mounts.
  add_github: Signal<bool>,
}

impl ProjectNav {
  pub fn show(&self, section: Section) {
    self.pipeline.update(|request| request.id = None);
    self.section.set(section);
  }

  /// Opens a pipeline in its editor, e.g. from an operation in the status bar.
  pub fn show_pipeline(&self, pipeline: ResourceId) {
    self.pipeline.update(|request| {
      request.revision += 1;
      request.id = Some(pipeline);
    });
    self.section.set(Section::Pipelines);
  }

  /// Opens the Agents screen at `profile`, e.g. from a connection's "Used by" link.
  pub fn show_profile(&self, profile: ResourceId) {
    self.agent.set(AgentSelection::Profile(profile));
    self.section.set(Section::Agents);
  }

  /// Opens Connections with Add GitHub connection open, e.g. for a remote on an Enterprise host.
  pub fn show_add_github(&self) {
    self.add_github.set(true);
    self.section.set(Section::Connections);
  }

  /// Whether Add GitHub connection was asked for; asking again needs another request.
  pub fn take_add_github(&self) -> bool {
    let requested = self.add_github.get_untracked();
    if requested {
      self.add_github.set(false);
    }
    requested
  }

  /// Opens a task's page.
  pub fn show_task(&self, task: TaskId) {
    self.show(Section::Task(task));
  }

  /// Tasks were saved; the shell reads them again.
  pub fn tasks_changed(&self) {
    self.revision.update(|revision| *revision += 1);
  }
}

/// The project's tasks as far as they are known.
#[derive(Clone, Debug, PartialEq)]
pub enum TasksState {
  Loading,
  /// Also kept while the tasks are read again after a change.
  Ready(ProjectTasks),
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

pub struct ProjectShell {
  nav: ProjectNav,
}

impl Component for ProjectShell {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      nav: ProjectNav {
        section: ctx.signal(Section::Overview),
        revision: ctx.signal(0),
        agent: ctx.signal(AgentSelection::First),
        pipeline: ctx.signal(PipelineRequest::default()),
        add_github: ctx.signal(false),
      },
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Provided on every render: a re-render from the root replaces this component's context
    // with the root's, which does not contain it.
    ctx.provide(self.nav.clone());
    let open = ctx.props::<OpenProject>().clone();
    let runners = ctx
      .use_context::<Runners>()
      .expect("the root provides Runners before mounting screens");
    let status = runner_status(ctx, &runners);
    let runner = status.state_for(&open.repo_root);
    let runs = run_changes(ctx, &runners);
    let tasks = self.tasks(ctx, &open, runs);
    let section = self.nav.section.get();
    let content = match section {
      Section::Overview => {
        let notice = runner::connection_notice(ctx, &runner, &runners);
        overview::overview(ctx, &open, notice, &tasks, &self.nav)
      }
      Section::Tasks => ctx.mount_keyed::<TasksScreen>(
        "tasks",
        TasksProps {
          project: open.project.name().as_str().into(),
          tasks: tasks.clone(),
        },
      ),
      Section::Pipelines => {
        let request = self.nav.pipeline.get();
        ctx.mount_keyed::<PipelinesScreen>(
          &format!("pipelines-{}", request.revision),
          PipelinesProps {
            project: open.clone(),
            open: request.id,
          },
        )
      }
      Section::NewTask => ctx.mount_keyed::<NewTaskForm>(
        "new-task",
        NewTaskProps {
          project: open.clone(),
          pipelines: tasks.ready().map(|tasks| tasks.pipelines.clone()),
        },
      ),
      Section::Task(task) => ctx.mount_keyed::<TaskScreen>(
        &format!("task-{task}"),
        TaskProps {
          project: open.clone(),
          task,
          revision: (self.nav.revision.get(), runs),
        },
      ),
      Section::PullRequests => ctx.mount_keyed::<PullRequestsScreen>("pull-requests", open.clone()),
      Section::PullRequest(number) => ctx.mount_keyed::<PullRequestScreen>(
        &format!("pull-request-{number}"),
        PullRequestProps {
          project: open.clone(),
          number,
        },
      ),
      Section::Agents => ctx.mount_keyed::<AgentsScreen>("agents", open.clone()),
      Section::Connections => ctx.mount_keyed::<ConnectionsScreen>("connections", open.clone()),
      Section::Settings => ctx.mount_keyed::<SettingsScreen>("settings", open.clone()),
    };
    let bar = StatusBarProps {
      project: Some(ProjectBar {
        repo_root: open.repo_root.clone(),
        runner: status,
        queued: tasks.ready().map(|tasks| list::queued(&tasks.tasks, &tasks.phases)),
      }),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .child(sidebar(ctx, &open, section, &tasks, &self.nav))
          .child(Column::new().flex(1.0).height(Dimension::Pct(100.0)).child(content)),
      )
      .child(ctx.mount_keyed::<StatusBar>("status-bar", bar))
  }
}

impl ProjectShell {
  /// The project's tasks, read again whenever tasks were saved or the runner changed a run.
  fn tasks(&self, ctx: &mut Ctx, open: &OpenProject, runs: u64) -> TasksState {
    let deps = (open.repo_root.clone(), self.nav.revision.get(), runs);
    let load = ctx.future(deps, |(repo_root, ..)| async move {
      background::run(move || ProjectTasks::load(&repo_root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot read the project's tasks");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    // A failed reload must not leave the previous tasks on screen as if they were current.
    match (state.data, state.error) {
      (_, Some(error)) => TasksState::Failed(error),
      (Some(tasks), None) => TasksState::Ready(tasks),
      (None, None) => TasksState::Loading,
    }
  }
}

/// Counts the runner's run change notifications, so screens showing runs read them again.
fn run_changes(ctx: &mut Ctx, runners: &Runners) -> u64 {
  let changes = ctx.stream(runners.clone(), |runners, emitter: StreamEmitter<u64, ()>| async move {
    let mut updates = runners.run_changes();
    loop {
      let revision = *updates.borrow_and_update();
      if !emitter.emit(revision) || updates.changed().await.is_err() {
        return;
      }
    }
  });
  changes.state().get().data.unwrap_or(0)
}

fn runner_status(ctx: &mut Ctx, runners: &Runners) -> RunnerStatus {
  let status = ctx.stream(
    runners.clone(),
    |runners, emitter: StreamEmitter<RunnerStatus, ()>| async move {
      let mut updates = runners.subscribe();
      loop {
        let status = updates.borrow_and_update().clone();
        if !emitter.emit(status) || updates.changed().await.is_err() {
          return;
        }
      }
    },
  );
  status.state().get().data.unwrap_or_else(|| runners.status())
}
