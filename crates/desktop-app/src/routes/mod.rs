//! Where the window can be: typed routes and the router paths they stand for. A path is all a tab's
//! history keeps, so everything a person can go back to is part of it, its query included.
//!
//! Paths: `/` is All projects, `/add-project` Add project (`?folder=` the folder it starts with) and
//! `/settings` the app's settings. A project's screens start at
//! `/project/<id>`, its Overview (`?tab=` the tab of its task list), and add the screen's segment, e.g.
//! `/project/<id>/pipelines`. The Tasks list is `/project/<id>/tasks` (`?tab=`, `?q=` the search,
//! `?highlight=` the task a Create just added) and New task `/project/<id>/tasks/new`. A task's page is
//! `/project/<id>/tasks/<key>/<tab>` (the key as in `KON-12`; `summary`, `conversation` or `changes`, with
//! `?file=` a changed file and `&from=worktree` for its change that is not checkpointed), always with
//! its tab, and `/project/<id>/tasks/<key>/edit` is Edit task. Connections is `/project/<id>/connections`
//! (`?tab=`, `?selected=` the connection its inspector shows), Add connection `/project/<id>/connections/new`
//! (`?kind=mcp_server` adds an MCP server) and `/project/<id>/connections/<connection id>/edit` edits an MCP
//! server. A tab, source or kind left out of the query is the one the screen opens on.
//!
//! Another screen or another tab (`tab`, a task's `<tab>`) is a new entry of the tab's history
//! (`Navigation::go`); what a screen refines on its tab, `q`, `highlight` and `file` with `from`, takes the
//! current entry's place (`Navigation::replace`), so Back leaves the screen instead of replaying it; so does
//! the connection Connections shows in its inspector (`selected`).
//!
//! A project is named by its id (`orchester_core::id::ProjectId`), not its key or name: the id never
//! changes, so a path a tab keeps still finds its project after a rename or a new key. A task is named
//! by its key (`orchester_core::project::TaskKey`), as people read it. A path that names no route reads
//! as a [`RouteError`] that says why.

mod accounts;
mod connections;
mod error;
mod overview;
mod page;
mod query;
mod screen;
mod table;
mod task;
mod tasks;
mod tools;

use std::path::{Path, PathBuf};

use orchester_core::id::ProjectId;

use self::query::{Params, with_query};
pub use self::{
  accounts::{AccountsPanel, AccountsView},
  connections::{ConnectionsTab, ConnectionsView, NewConnection},
  error::RouteError,
  overview::OverviewTab,
  page::ProjectPage,
  screen::Screen,
  table::routes,
  task::{ChangeSource, ChangedFile, TaskTab, TaskView},
  tasks::{TaskList, TaskListTab},
  tools::ToolTab,
};

const PROJECT: &str = "project";
const SETTINGS: &str = "settings";
const ADD_PROJECT: &str = "add-project";
/// The query parameter of the folder Add project starts with.
const FOLDER: &str = "folder";
const TASKS: &str = "tasks";
const CONNECTIONS: &str = "connections";
const NEW: &str = "new";
const EDIT: &str = "edit";
/// The query parameter of a screen's tab.
const TAB: &str = "tab";
/// Settings' Accounts tab.
const ACCOUNTS: &str = "accounts";
/// A project's path with its identifier as the router's parameter.
const PROJECT_PATTERN: &str = "/project/:project";
const NEW_PIPELINE_PATTERN: &str = "/project/:project/pipelines/new";
const PIPELINE_PATTERN: &str = "/project/:project/pipelines/:pipeline";
const AGENT_PATTERN: &str = "/project/:project/agents/:agent";

/// A folder a route names. An address is text, so only a folder whose path is text can be named: the
/// address then keeps it whole.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RouteFolder(String);

impl RouteFolder {
  /// `folder`, or `None` when its path is not text (possible on Windows and Unix file systems).
  pub fn new(folder: &Path) -> Option<Self> {
    folder.to_str().map(|path| Self(path.to_owned()))
  }

  pub fn path(&self) -> PathBuf {
    PathBuf::from(&self.0)
  }

  fn as_str(&self) -> &str {
    &self.0
  }
}

/// A place in the window.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Route {
  AllProjects,
  /// Add project, with the folder it starts with, e.g. one Open folder… found no project in.
  AddProject(Option<RouteFolder>),
  Settings,
  /// Settings › Accounts, with the account it shows and its panel.
  Accounts(AccountsView),
  Project(ProjectId, ProjectPage),
}

impl Route {
  /// A pipeline's list, creation page and editor belong to one sidebar screen, with distinct paths.
  pub fn pattern(&self) -> String {
    match self {
      Self::Project(_, ProjectPage::NewPipeline | ProjectPage::NewEmptyPipeline) => NEW_PIPELINE_PATTERN.to_owned(),
      Self::Project(_, ProjectPage::Pipeline(_)) => PIPELINE_PATTERN.to_owned(),
      Self::Project(_, ProjectPage::Agent(_)) => AGENT_PATTERN.to_owned(),
      _ => self.screen().pattern(),
    }
  }
  pub const fn screen(&self) -> Screen {
    match self {
      Self::AllProjects => Screen::AllProjects,
      Self::AddProject(_) => Screen::AddProject,
      Self::Settings | Self::Accounts(_) => Screen::Settings,
      Self::Project(_, page) => page.screen(),
    }
  }

  pub const fn project(&self) -> Option<ProjectId> {
    match self {
      Self::Project(project, _) => Some(*project),
      Self::AllProjects | Self::AddProject(_) | Self::Settings | Self::Accounts(_) => None,
    }
  }

  /// The screen the sidebar shows as selected: a task and its forms belong to Tasks.
  pub const fn nav_screen(&self) -> Screen {
    match self.screen() {
      Screen::Task | Screen::NewTask | Screen::EditTask => Screen::Tasks,
      Screen::NewConnection | Screen::EditConnection => Screen::Connections,
      screen => screen,
    }
  }

  pub fn path(&self) -> String {
    match self {
      Self::AllProjects => "/".to_owned(),
      Self::AddProject(folder) => with_query(
        format!("/{ADD_PROJECT}"),
        &[(FOLDER, folder.as_ref().map(RouteFolder::as_str))],
      ),
      Self::Settings => format!("/{SETTINGS}"),
      Self::Accounts(view) => view.path(format!("/{SETTINGS}"), (TAB, ACCOUNTS)),
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
      [ADD_PROJECT] => Self::AddProject(params.take(FOLDER)?.map(RouteFolder)),
      [SETTINGS] => Self::settings(&mut params)?,
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

impl Route {
  /// Settings at the tab its query names: Automation, or Accounts with the account it shows.
  fn settings(params: &mut Params) -> Result<Self, RouteError> {
    match params.take(TAB)?.as_deref() {
      None => Ok(Self::Settings),
      Some(ACCOUNTS) => AccountsView::take(params).map(Self::Accounts),
      Some(other) => Err(RouteError::InvalidParam {
        param: TAB.to_owned(),
        value: other.to_owned(),
      }),
    }
  }
}

fn project_screen_path(project_root: &str, screen: Screen) -> String {
  match screen.segment() {
    Some(segment) => format!("{project_root}/{segment}"),
    None => project_root.to_owned(),
  }
}
