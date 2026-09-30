//! Project shell: the sidebar (`zo9Py`) beside the open project screen — the overview
//! (`Twtqc`), the task list (`J9467`), the new task form (`l2YA4`), a task (`RD4pc`, `SfXU1`),
//! pull requests (`wGtxT`) and one pull request
//! (`Rrmhf`), instructions (`yRKeF`), agent profiles (`WPmtS`), tools (`ZFwWl`, `YEWFo`), secrets (`R99lt`),
//! connections (`a4Lqb`) or Project settings (`QV1af`), above the status bar (`pmOpt`). The router
//! renders the screen the path names into the shell's outlet. The shell hands the project's tasks,
//! usage and runner, read once per project for every tab of it (`tabs::feed`), to the sidebar, the
//! screen and the status bar.

mod agents;
pub(crate) mod connections;
mod import;
mod instructions;
mod new_task;
mod overview;
mod page;
mod pages;
mod pipelines;
mod pull_requests;
mod repository_import;
mod runner;
mod runner_card;
mod secrets;
mod settings;
mod sidebar;
mod skills;
mod switcher;
mod task;
mod task_rows;
mod tasks;
mod tools;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::{Column, Outlet, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
  router::Navigator,
};
use orchester_core::id::{ProjectId, RunId, TaskId};

pub(crate) use self::pages::page;
use self::{
  pages::Shown,
  sidebar::{SidebarState, sidebar},
};
use super::status_bar::{ProjectBar, StatusBar, StatusBarProps};
use crate::{
  messages::Message,
  navigation::{
    Navigation, PROJECT_PARAM, entry,
    import::{self as import_query, ImportTab},
    repository_import as repository_query,
    route::{self, ChangedFile, Page, Route},
  },
  runner::{RunnerStatus, Runners, state::RunnerState},
  tabs::{
    OpenIn, Tabs,
    feed::{ProjectFeed, ProjectFeeds},
  },
  tasks::{data::ProjectTasks, integration_jobs::RunIntegration, list, overview::OverviewTab},
  ui::link::Link,
  workspace::OpenProject,
};

/// Provided by the shell: going to a page of the project and announcing that tasks changed.
#[derive(Clone)]
pub struct ProjectNav {
  navigator: Navigator,
  project: ProjectId,
  /// The project's data, shared with every other tab of it.
  feed: ProjectFeed,
  /// Connections opens Add GitHub connection when it next mounts.
  add_github: Signal<bool>,
  /// Why the task an Edit task page was opened for can no longer be edited; its page, shown
  /// instead, says so.
  refused_edit: Signal<Option<(TaskId, Message)>>,
  tabs: Tabs,
}

impl ProjectNav {
  /// Opens `page` as a new history entry, as every user navigation does, once the page shown may
  /// be left ([`Tabs::leave_active`]).
  pub fn show(&self, page: Page) {
    let (navigator, path) = (self.navigator.clone(), page.path(self.project));
    self.tabs.leave_active(move || entry::push(&navigator, path.clone()));
  }

  /// Opens `page` where `open` says: in this tab as a new history entry, or in a new tab.
  pub fn open(&self, page: Page, open: OpenIn) {
    match open {
      OpenIn::Current => self.show(page),
      OpenIn::Background | OpenIn::NewTab => self.tabs.open(Route::Project(self.project, page), open),
    }
  }

  pub fn tabs(&self) -> &Tabs {
    &self.tabs
  }

  /// A link to `page` (see `ui::link`).
  pub fn link(&self, page: Page) -> Link {
    let nav = self.clone();
    Link::new(&self.tabs, move |open| nav.open(page, open))
  }

  /// A link to `task`'s Changes tab on `file`; in this tab it is a new history entry.
  pub fn changes_link(&self, task: TaskId, file: &ChangedFile) -> Link {
    let (nav, path) = (self.clone(), route::changes_path(self.project, task, file));
    Link::new(&self.tabs, move |open| match open {
      OpenIn::Current => entry::push(&nav.navigator, path.clone()),
      OpenIn::Background | OpenIn::NewTab => nav.tabs.open_path(path.clone(), open),
    })
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
    if !self.opened_from(&page) {
      self.show(page);
      return;
    }
    let navigator = self.navigator.clone();
    self.tabs.leave_active(move || {
      navigator.back();
    });
  }

  /// Whether the current entry was opened from the screen `page` shows, on any tab.
  fn opened_from(&self, page: &Page) -> bool {
    matches!(
      entry::from(&self.navigator),
      Some(Route::Project(project, from)) if project == self.project && from.same_screen(page)
    )
  }

  /// Opens Import resources over the page shown, on `tab`, as a new history entry.
  pub fn show_import(&self, tab: ImportTab) {
    let path = self.navigator.path().get_untracked();
    entry::push(&self.navigator, import_query::open(&path, tab));
  }

  /// Whether Import resources is open over the page shown.
  pub fn import_shown(&self) -> bool {
    import_query::tab(&self.navigator.path().get_untracked()).is_some()
  }

  /// Closes Import resources: back to the entry it was opened from when that shows the page under
  /// it, otherwise that page in its place.
  pub fn close_import(&self) {
    let under = import_query::close(&self.navigator.path().get_untracked());
    let from = entry::from(&self.navigator);
    if from.is_some() && from == Route::parse(&under) {
      self.navigator.back();
    } else {
      entry::replace(&self.navigator, under);
    }
  }

  /// Opens Import from repository over the page shown.
  pub fn show_repository_import(&self) {
    let path = self.navigator.path().get_untracked();
    entry::push(&self.navigator, repository_query::open(&path));
  }

  /// Whether Import from repository is open over the page shown.
  pub fn repository_import_shown(&self) -> bool {
    repository_query::shown(&self.navigator.path().get_untracked())
  }

  /// Closes Import from repository, as [`Self::close_import`] closes Import resources.
  pub fn close_repository_import(&self) {
    let under = repository_query::close(&self.navigator.path().get_untracked());
    let from = entry::from(&self.navigator);
    if from.is_some() && from == Route::parse(&under) {
      self.navigator.back();
    } else {
      entry::replace(&self.navigator, under);
    }
  }

  /// Leaves Import from repository for `page`, in its place: the dialog's entry is not visited
  /// again.
  pub fn leave_repository_import(&self, page: Page) {
    entry::replace(&self.navigator, page.path(self.project));
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

  /// Edit task was opened for `task`, which can no longer be edited (`reason`): its page takes the
  /// form's place and says why.
  pub fn refuse_edit(&self, task: TaskId, reason: Message) {
    self.refused_edit.set(Some((task, reason)));
    self.replace(Page::task(task));
  }

  /// Why Edit task was refused for `task`, once: the task page that took the form's place shows
  /// it.
  pub fn take_refused_edit(&self, task: TaskId) -> Option<Message> {
    let (refused, reason) = self.refused_edit.get_untracked()?;
    if refused != task {
      return None;
    }
    self.refused_edit.set(None);
    Some(reason)
  }

  /// Tasks were saved; every tab of the project reads them again.
  pub fn tasks_changed(&self) {
    self.feed.revision.update(|revision| *revision += 1);
  }

  /// The current operation for a run, shared across every tab of this project.
  pub fn integration(&self, run: RunId) -> RunIntegration {
    self.feed.integration.of(run)
  }

  /// Resources were imported: the screens that list pipelines, profiles, instructions or tools
  /// start over and read them again, in every tab of the project.
  pub fn resources_changed(&self) {
    self.feed.resources.update(|revision| *revision += 1);
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
  // Going back and forward passes no guard: an entry of a project removed from the workspace
  // gives way to the start screen.
  let Some(open) = open else {
    let start = navigation.places().start().unwrap_or(Route::Start);
    navigation.router().replace(start.path());
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
    let project = ctx.props::<OpenProject>().project.id();
    let feeds = ctx
      .use_context::<ProjectFeeds>()
      .expect("the window provides the project feeds before mounting screens");
    Self {
      nav: ProjectNav {
        navigator: ctx.navigator().expect("the router mounts the project shell"),
        project,
        feed: feeds.of(project),
        add_github: ctx.signal(false),
        refused_edit: ctx.signal(None),
        tabs: ctx
          .use_context::<Tabs>()
          .expect("the window provides Tabs before mounting screens"),
      },
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // Provided on every render: a re-render from the root replaces this component's context
    // with the root's, which does not contain it.
    ctx.provide(self.nav.clone());
    let open = ctx.props::<OpenProject>().clone();
    let feed = &self.nav.feed;
    let status = feed.status.get().unwrap_or_else(RunnerStatus::none);
    let runner = status.state_for(&open.repo_root);
    let tasks = feed.tasks.get().without_runner(runner == RunnerState::Online);
    let pending = tasks
      .ready()
      .map(|tasks| tasks.pending_tool_approvals.as_slice())
      .unwrap_or_default();
    let usage = feed.usage.get();
    let page = Route::current_page(ctx);
    ctx.provide(Shown {
      open: open.clone(),
      status: status.clone(),
      runner,
      revision: (feed.revision.get(), feed.changes.get()),
      resources: feed.resources.get(),
      tasks: tasks.clone(),
      usage,
    });
    let bar = ProjectBar {
      project_name: open.project.name().as_str().to_owned(),
      connections: feed.connections.get().0,
      repo_root: open.repo_root.clone(),
      runner: status.clone(),
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
          .child(sidebar(
            ctx,
            &open,
            &SidebarState {
              shown: page,
              tasks: &tasks,
              runner: &status,
            },
            &self.nav,
          ))
          .child(
            Column::new()
              .flex(1.0)
              .height(Dimension::Pct(100.0))
              .child(Outlet::mount(ctx)),
          ),
      )
      .child(ctx.mount_keyed::<StatusBar>("status-bar", StatusBarProps { project: Some(bar) }))
      .child(import::dialog(ctx, &open))
      .child(repository_import::dialog(ctx, &open))
      .with_children(task::tool_approval_modal(ctx, pending))
  }
}
