//! Project shell: the sidebar (`zo9Py`) beside the open project screen — the overview
//! (`Twtqc`), the task list (`J9467`), the new task form (`l2YA4`), agent profiles (`WPmtS`)
//! or connections (`a4Lqb`). The shell reads the project's tasks once and hands them to the
//! sidebar and the screen.

mod agents;
mod connections;
mod new_task;
mod overview;
mod page;
mod runner;
mod sidebar;
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
use orchester_core::id::ResourceId;

use self::{
  agents::AgentsScreen,
  connections::ConnectionsScreen,
  new_task::{NewTaskForm, NewTaskProps},
  sidebar::sidebar,
  tasks::{TasksProps, TasksScreen},
};
use crate::{
  background,
  messages::{self, Message},
  runner::{RunnerStatus, Runners},
  tasks::data::ProjectTasks,
  workspace::OpenProject,
};

/// The screen shown beside the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
  Overview,
  Tasks,
  NewTask,
  Agents,
  Connections,
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

/// Provided by the shell: switching screens and announcing that tasks changed.
#[derive(Clone)]
pub struct ProjectNav {
  section: Signal<Section>,
  revision: Signal<u64>,
  agent: Signal<AgentSelection>,
}

impl ProjectNav {
  pub fn show(&self, section: Section) {
    self.section.set(section);
  }

  /// Opens the Agents screen at `profile`, e.g. from a connection's "Used by" link.
  pub fn show_profile(&self, profile: ResourceId) {
    self.agent.set(AgentSelection::Profile(profile));
    self.section.set(Section::Agents);
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
    let tasks = self.tasks(ctx, &open);
    let section = self.nav.section.get();
    let content = match section {
      Section::Overview => {
        let notice = runner::connection_notice(ctx, &status, &runner, &runners);
        overview::overview(ctx, &open, notice, &tasks, &self.nav)
      }
      Section::Tasks => ctx.mount_keyed::<TasksScreen>(
        "tasks",
        TasksProps {
          project: open.project.name().as_str().into(),
          tasks: tasks.clone(),
        },
      ),
      Section::NewTask => ctx.mount_keyed::<NewTaskForm>(
        "new-task",
        NewTaskProps {
          project: open.clone(),
          pipelines: tasks.ready().map(|tasks| tasks.pipelines.clone()),
        },
      ),
      Section::Agents => ctx.mount_keyed::<AgentsScreen>("agents", open.clone()),
      Section::Connections => ctx.mount_keyed::<ConnectionsScreen>("connections", open.clone()),
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(sidebar(ctx, &open, &runner, section, &tasks, &self.nav))
      .child(Column::new().flex(1.0).height(Dimension::Pct(100.0)).child(content))
  }
}

impl ProjectShell {
  fn tasks(&self, ctx: &mut Ctx, open: &OpenProject) -> TasksState {
    let deps = (open.repo_root.clone(), self.nav.revision.get());
    let load = ctx.future(deps, |(repo_root, _)| async move {
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
