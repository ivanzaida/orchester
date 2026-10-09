//! Where the window can be: typed routes and the router paths they stand for. A path is all a tab's
//! history keeps, so everything a person can go back to is part of it, its query included.
//!
//! Paths: `/` is All projects and `/settings` the app's settings. A project's screens start at
//! `/project/<id>`, its Overview (`?tab=` the tab of its task list), and add the screen's segment, e.g.
//! `/project/<id>/pipelines`. The Tasks list is `/project/<id>/tasks` (`?tab=`, `?q=` the search,
//! `?highlight=` the task a Create just added) and New task `/project/<id>/tasks/new`. A task's page is
//! `/project/<id>/tasks/<key>/<tab>` (the key as in `KON-12`; `summary`, `conversation` or `changes`, with
//! `?file=` a changed file and `&from=worktree` for its change that is not checkpointed), always with
//! its tab, and `/project/<id>/tasks/<key>/edit` is Edit task. A tab or source left out of the query is
//! the one the screen opens on.
//!
//! Another screen or another tab (`tab`, a task's `<tab>`) is a new entry of the tab's history
//! (`Navigation::go`); what a screen refines on its tab, `q`, `highlight` and `file` with `from`, takes the
//! current entry's place (`Navigation::replace`), so Back leaves the screen instead of replaying it.
//!
//! A project is named by its id (`orchester_core::id::ProjectId`), not its key or name: the id never
//! changes, so a path a tab keeps still finds its project after a rename or a new key. A task is named
//! by its key (`orchester_core::project::TaskKey`), as people read it. A path that names no route reads
//! as a [`RouteError`] that says why.

mod error;
mod overview;
mod page;
mod query;
mod screen;
mod table;
mod task;
mod tasks;

use orchester_core::id::ProjectId;

use self::query::Params;
pub use self::{
  error::RouteError,
  overview::OverviewTab,
  page::ProjectPage,
  screen::Screen,
  table::routes,
  task::{ChangeSource, ChangedFile, TaskTab, TaskView},
  tasks::{TaskList, TaskListTab},
};

const PROJECT: &str = "project";
const SETTINGS: &str = "settings";
const TASKS: &str = "tasks";
const NEW: &str = "new";
const EDIT: &str = "edit";
/// The query parameter of a screen's tab.
const TAB: &str = "tab";
/// A project's path with its identifier as the router's parameter.
const PROJECT_PATTERN: &str = "/project/:project";

/// A place in the window.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Route {
  AllProjects,
  Settings,
  Project(ProjectId, ProjectPage),
}

impl Route {
  pub const fn screen(&self) -> Screen {
    match self {
      Self::AllProjects => Screen::AllProjects,
      Self::Settings => Screen::Settings,
      Self::Project(_, page) => page.screen(),
    }
  }

  pub const fn project(&self) -> Option<ProjectId> {
    match self {
      Self::Project(project, _) => Some(*project),
      Self::AllProjects | Self::Settings => None,
    }
  }

  /// The screen the sidebar shows as selected: a task and its forms belong to Tasks.
  pub const fn nav_screen(&self) -> Screen {
    match self.screen() {
      Screen::Task | Screen::NewTask | Screen::EditTask => Screen::Tasks,
      screen => screen,
    }
  }

  pub fn path(&self) -> String {
    match self {
      Self::AllProjects => "/".to_owned(),
      Self::Settings => format!("/{SETTINGS}"),
      Self::Project(project, page) => page.path(&format!("/{PROJECT}/{project}")),
    }
  }

  /// The route at `path`, query included. The path is matched first, so an address that names no screen
  /// says so whatever its query holds.
  pub fn parse(path: &str) -> Result<Self, RouteError> {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let mut params = Params::new(query);
    let rest = path.strip_prefix('/').ok_or(RouteError::NoScreen)?;
    let segments: Vec<&str> = if rest.is_empty() {
      Vec::new()
    } else {
      rest.split('/').collect()
    };
    // A trailing `/` or a `//` leaves an empty segment, which no screen's path has.
    if segments.contains(&"") {
      return Err(RouteError::NoScreen);
    }
    let route = match segments.as_slice() {
      [] => Self::AllProjects,
      [SETTINGS] => Self::Settings,
      [PROJECT, project, page @ ..] => {
        let project = project.parse().map_err(|_| RouteError::ProjectId {
          value: (*project).to_owned(),
        })?;
        Self::Project(project, ProjectPage::parse(page, &mut params)?)
      }
      _ => return Err(RouteError::NoScreen),
    };
    params.finish()?;
    Ok(route)
  }
}

fn project_screen_path(project_root: &str, screen: Screen) -> String {
  match screen.segment() {
    Some(segment) => format!("{project_root}/{segment}"),
    None => project_root.to_owned(),
  }
}
