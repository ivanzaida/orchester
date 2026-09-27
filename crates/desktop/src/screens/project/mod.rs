//! Project shell: the sidebar (`zo9Py`) beside the open project screen — the overview
//! (`Twtqc`), the task list (`J9467`), the new task form (`l2YA4`), a task (`RD4pc`, `SfXU1`),
//! pull requests (`wGtxT`) and one pull request
//! (`Rrmhf`), instructions (`yRKeF`), agent profiles (`WPmtS`), secrets (`R99lt`),
//! connections (`a4Lqb`) or Project settings (`QV1af`), above the status bar (`pmOpt`). The router
//! renders the screen the path names into the shell's outlet. The shell reads the project's
//! tasks once and hands them to the sidebar, the screen and the status bar.

mod agents;
pub(crate) mod connections;
mod instructions;
mod new_task;
mod overview;
mod page;
mod pages;
mod pipelines;
mod pull_requests;
mod runner;
mod secrets;
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
  components::{Column, Outlet, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
  router::Navigator,
};
use orchester_core::{
  id::{ProjectId, TaskId},
  time::Timestamp,
};

pub(crate) use self::pages::page;
use self::{pages::Shown, sidebar::sidebar};
use super::status_bar::{ProjectBar, StatusBar, StatusBarProps};
use crate::{
  background,
  messages::{self, Message},
  navigation::{
    Navigation, PROJECT_PARAM, entry,
    route::{self, ChangedFile, Page, Route},
  },
  runner::{RunnerStatus, Runners, state::RunnerState},
  tasks::{data::ProjectTasks, list, overview::OverviewTab, usage::ProjectUsage},
  workspace::OpenProject,
};

/// Provided by the shell: going to a page of the project and announcing that tasks changed.
#[derive(Clone)]
pub struct ProjectNav {
  navigator: Navigator,
  project: ProjectId,
  revision: Signal<u64>,
  /// Connections opens Add GitHub connection when it next mounts.
  add_github: Signal<bool>,
}

impl ProjectNav {
  /// Opens `page` as a new history entry, as every user navigation does.
  pub fn show(&self, page: Page) {
    entry::push(&self.navigator, page.path(self.project));
  }

  /// Opens `task`'s Changes tab on `file`, as a new history entry.
  pub fn show_changes(&self, task: TaskId, file: &ChangedFile) {
    entry::push(&self.navigator, route::changes_path(self.project, task, file));
  }

  /// Shows `file` on `task`'s Changes tab in place of the current entry: choosing another file
  /// does not add to history.
  pub fn replace_changes(&self, task: TaskId, file: &ChangedFile) {
    entry::replace(&self.navigator, route::changes_path(self.project, task, file));
  }

  /// Shows `page` in place of the current entry, when the selection follows a save, so going
  /// back does not return to the stale state.
  pub fn replace(&self, page: Page) {
    entry::replace(&self.navigator, page.path(self.project));
  }

  /// Leaves a submitted form for `page`. When the form was opened from the screen `page` shows,
  /// history goes back to that entry, so the screen does not follow itself in history;
  /// otherwise `page` takes the form's place. Either way the form is not visited again.
  pub fn finish(&self, page: Page) {
    if self.opened_from(&page) {
      entry::finish_back(&self.navigator);
    } else {
      self.replace(page);
    }
  }

  /// Leaves a form without submitting it, for `page`. When the form was opened from the screen
  /// `page` shows, history goes back to that entry, on the tab it was on; otherwise `page` opens
  /// as a new entry. The form stays in history either way, as any screen left unfinished does.
  pub fn leave(&self, page: Page) {
    if self.opened_from(&page) {
      self.navigator.back();
    } else {
      self.show(page);
    }
  }

  /// Whether the current entry was opened from the screen `page` shows, on any tab.
  fn opened_from(&self, page: &Page) -> bool {
    matches!(
      entry::from(&self.navigator),
      Some(Route::Project(project, from)) if project == self.project && from.same_screen(page)
    )
  }

  /// Opens Connections with Add GitHub connection open, e.g. for a remote on an Enterprise host.
  pub fn show_add_github(&self) {
    self.add_github.set(true);
    self.show(Page::CONNECTIONS);
  }

  /// Whether Add GitHub connection was asked for; asking again needs another request.
  pub fn take_add_github(&self) -> bool {
    let requested = self.add_github.get_untracked();
    if requested {
      self.add_github.set(false);
    }
    requested
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
  /// The tasks as they stand while the project's runner is `online` or not: without a runner,
  /// runs recorded as running are interrupted.
  fn without_runner(self, online: bool) -> Self {
    match self {
      Self::Ready(tasks) if !online => Self::Ready(tasks.without_runner()),
      other => other,
    }
  }

  pub fn ready(&self) -> Option<&ProjectTasks> {
    match self {
      Self::Ready(tasks) => Some(tasks),
      Self::Loading | Self::Failed(_) => None,
    }
  }
}

/// The project layout route: the shell of the project the path names. The route guard lets only
/// projects opened in this session through.
pub(crate) fn shell(ctx: &mut Ctx) -> Element {
  let navigation = ctx
    .use_context::<Navigation>()
    .expect("the root provides Navigation before mounting screens");
  let open = ctx
    .route_params()
    .get_parsed::<ProjectId>(PROJECT_PARAM)
    .and_then(|project| navigation.places().project(project));
  let Some(open) = open else {
    return Element::new();
  };
  // A project's runner starts as soon as the project is shown, at startup or after setup, and
  // keeps running in the background when another screen replaces it. Repeating the same project
  // is a no-op.
  ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens")
    .show(Some(&open));
  ctx.mount_keyed::<ProjectShell>(&format!("project-{}", open.project.id()), open)
}

pub struct ProjectShell {
  nav: ProjectNav,
}

impl Component for ProjectShell {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      nav: ProjectNav {
        navigator: ctx.navigator().expect("the router mounts the project shell"),
        project: ctx.props::<OpenProject>().project.id(),
        revision: ctx.signal(0),
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
    let tasks = self
      .tasks(ctx, &open, runs)
      .without_runner(runner == RunnerState::Online);
    let usage = self.usage(ctx, &open, runs);
    let page = Route::current_page(ctx);
    ctx.provide(Shown {
      open: open.clone(),
      status: status.clone(),
      runner,
      revision: (self.nav.revision.get(), runs),
      tasks: tasks.clone(),
      usage,
    });
    let bar = ProjectBar {
      repo_root: open.repo_root.clone(),
      runner: status,
      queued: tasks.ready().map(|tasks| list::queued(&tasks.tasks, &tasks.phases)),
      needs_you: tasks
        .ready()
        .map(|tasks| OverviewTab::NeedsYou.count(&tasks.tasks, &tasks.phases)),
      usage,
      lost_to_runner: tasks.ready().map(|tasks| tasks.lost_to_runner),
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .child(sidebar(ctx, &open, page, &tasks, &self.nav))
          .child(
            Column::new()
              .flex(1.0)
              .height(Dimension::Pct(100.0))
              .child(Outlet::mount(ctx)),
          ),
      )
      .child(ctx.mount_keyed::<StatusBar>("status-bar", StatusBarProps { project: Some(bar) }))
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

impl ProjectShell {
  /// The project's limits and today's usage, read again whenever settings or runs changed. `None`
  /// until read; a failed read is logged and leaves it unknown.
  fn usage(&self, ctx: &mut Ctx, open: &OpenProject, runs: u64) -> Option<ProjectUsage> {
    let day_start = Timestamp::now().local_day_start();
    let deps = (open.repo_root.clone(), self.nav.revision.get(), runs, day_start);
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
