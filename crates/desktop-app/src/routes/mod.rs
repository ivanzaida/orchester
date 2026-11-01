//! Where the window can be: typed routes and the router paths they stand for. A path is all a tab's
//! history keeps, so everything a person can go back to is part of it, its query included.
//!
//! Paths: `/` is All projects, `/add-project` Add project (`?folder=` the folder it starts with), `/join-team` Join a
//! team (`?way=code` with a code, `?way=projects` a joined team's shared projects, `?way=folder` linking one to a
//! folder, [`JoinWay`]), `/first-download?project=<id>` a shared project's first download and
//! `/settings` the app's settings (`?tab=general`, with `&show=updates` or `whats-new` its *Updates* in view,
//! [`UpdatesShow`], or `&show=plan-limits` its *Plan limits*; `notifications`, `accounts` or `orchester` for Orchester
//! Cloud; Automation without one, and
//! `?ask=rotate-token` asks *Rotate the token?* there). A project's screens start at
//! `/project/<id>`, its Overview (`?tab=` the tab of its task list), and add the screen's segment, e.g.
//! `/project/<id>/pipelines`. The Tasks list is `/project/<id>/tasks` (`?tab=`, `?q=` the search,
//! `?highlight=` the task a Create just added) and New task `/project/<id>/tasks/new`. A task's page is
//! `/project/<id>/tasks/<key>/<tab>` (the key as in `KON-12`; `summary`, `conversation` or `changes`, with
//! `?file=` a changed file and `&from=worktree` for its change that is not checkpointed), written always with
//! its tab and read without one as its Summary, and `/project/<id>/tasks/<key>/edit` is Edit task.
//! Modules is `/project/<id>/modules` (`?tab=closed`), New module `/project/<id>/modules/new`, a module's page
//! `/project/<id>/modules/<module id>` (its Summary; `/burndown` after it its Burndown) and Edit module
//! `/project/<id>/modules/<module id>/edit` ([`ModuleListTab`], [`ModuleTab`]). The Delivery view
//! beside Usage is `/project/<id>/usage/delivery` (`?period=` 30d, 90d or 365d).
//! Project settings is `/project/<id>/settings` (`?tab=runs`, [`ProjectSettingsTab`];
//! `?tab=runs&view=rules` its page *Approved for every run*). Its Connections tab is `/project/<id>/connections`
//! (`?selected=` the connection whose page shows), Add provider `/project/<id>/connections/new`
//! (`?kind=mcp_server` adds an MCP server) and `/project/<id>/connections/<connection id>/edit` edits an MCP server or
//! an agent CLI connection's account; its Secrets tab is `/project/<id>/secrets` ([`SecretsView`]). The code viewer is
//! `/project/<id>/files`, a file `/project/<id>/files/<path>` (`?at=` its version, `&line=` its lines; see
//! [`FilesView`]). Pull requests is `/project/<id>/pull-requests` (`?tab=`) and a pull request
//! `/project/<id>/pull-requests/<number>` (`?file=` the file whose diff it shows). Import resources is
//! `/project/<id>/import` (`?tab=` its kind tab, [`ImportTab`]). The Skills
//! library, an agent profile, Instructions, a pipeline's page and Memory keep their tab,
//! filter, item and panel in the query too ([`SkillsFilter`], [`AgentTab`],
//! [`InstructionsView`], [`PipelineView`], [`MemoryTab`]);
//! `/project/<id>/settings?tab=sharing&ask=make-local` opens Sharing with *Make local*'s question. A tab, source
//! or kind left out of the query is the one the screen opens on.
//!
//! Every place a person chooses — another screen, tab, filter, item or panel — is a new entry of the tab's
//! history (`Navigation::go`), so Back returns to it as in a browser. Only what chooses no place takes the
//! current entry's place (`Navigation::replace`): a route the app corrects (`highlight` after a Create, a
//! saved draft's id, a removed item) and the Tasks list's search `q` as it is typed; a closed panel leaves
//! no entry behind (`Navigation::close_panel`).
//!
//! A project is named by its id (`orchester_core::id::ProjectId`), not its key or name: the id never
//! changes, so a path a tab keeps still finds its project after a rename or a new key. A task is named
//! by its key (`orchester_core::project::TaskKey`), as people read it. A path that names no route reads
//! as a [`RouteError`] that says why.

mod accounts;
mod agents;
mod connections;
mod error;
mod files;
mod import;
mod instructions;
mod join;
mod join_team;
mod memory;
mod modules;
mod orchester;
mod orchestrator;
mod overview;
mod page;
mod pipeline;
mod project_settings;
mod query;
mod screen;
mod secrets;
mod skills;
mod table;
mod task;
mod tasks;
mod tools;
mod updates;
mod usage;

use std::path::{Path, PathBuf};

use orchester_core::id::ProjectId;
use orchester_workspace::usage::UsageQuery;

use self::query::{Params, with_query};
pub use self::{
  accounts::{AccountsPanel, AccountsView},
  agents::AgentTab,
  connections::{ConnectionsView, NewConnection},
  error::RouteError,
  files::FilesView,
  import::ImportTab,
  instructions::{InstructionsPanel, InstructionsTab, InstructionsView},
  join::JoinTarget,
  join_team::JoinWay,
  memory::MemoryTab,
  modules::{ModuleListTab, ModuleTab},
  orchester::{ConnectStep, OrchesterAsk, OrchesterPanel, OrchesterView},
  orchestrator::OrchestratorChat,
  overview::OverviewTab,
  page::ProjectPage,
  pipeline::{PipelineTab, PipelineView, StepPage},
  project_settings::ProjectSettingsTab,
  screen::Screen,
  secrets::{SecretsPanel, SecretsView},
  skills::SkillsFilter,
  table::routes,
  task::{ChangeSource, ChangedFile, TaskTab, TaskView},
  tasks::{TaskList, TaskListTab},
  tools::ToolTab,
  updates::UpdatesShow,
};

const PROJECT: &str = "project";
const SETTINGS: &str = "settings";
const ADD_PROJECT: &str = "add-project";
/// A shared project's first download; `?project=` names it.
const FIRST_DOWNLOAD: &str = "first-download";
/// The query parameter of the folder Add project starts with.
const FOLDER: &str = "folder";
const TASKS: &str = "tasks";
const CONNECTIONS: &str = "connections";
const PULL_REQUESTS: &str = "pull-requests";
/// Import resources' segment.
const IMPORT: &str = "import";
const ORCHESTRATOR: &str = "orchestrator";
const NEW: &str = "new";
const EDIT: &str = "edit";
/// The query parameter of a screen's tab.
const TAB: &str = "tab";
/// Settings' General tab.
const GENERAL: &str = "general";
/// Settings' Notifications tab.
const NOTIFICATIONS: &str = "notifications";
/// Settings' Browser tab.
const BROWSER: &str = "browser";
/// Settings' Accounts tab.
const ACCOUNTS: &str = "accounts";
/// Settings' Orchester Cloud tab, by the id it kept from when it was named Orchester.
const ORCHESTER: &str = "orchester";
/// Automation's question in place: *Rotate the token?* (`shpsk`).
const ASK: &str = "ask";
const ROTATE_TOKEN: &str = "rotate-token";
/// The invite landing under Settings › Orchester.
const INVITE: &str = "invite";
const INVITE_PATTERN: &str = "/settings/invite";
/// A project's path with its identifier as the router's parameter.
const PROJECT_PATTERN: &str = "/project/:project";
const NEW_PIPELINE_PATTERN: &str = "/project/:project/pipelines/new";
const PIPELINE_PATTERN: &str = "/project/:project/pipelines/:pipeline";
const AGENT_PATTERN: &str = "/project/:project/agents/:agent";
/// A file of the code viewer: its path is the rest of the address, however many segments it has.
const FILE_PATTERN: &str = "/project/:project/files/**path";
/// A file's route under the code viewer's layout (`table`): [`FILE_PATTERN`] after the screen's own.
const FILE_REST: &str = "/**path";
const PULL_REQUEST_PATTERN: &str = "/project/:project/pull-requests/:number";
/// The Modules list's segment (`orchester_workspace::links::MODULES`); New module and a module's page without its tab
/// under it, and a module's page on its tab and Edit module under that.
const MODULES: &str = orchester_workspace::links::MODULES;
const MODULE_PATTERN: &str = "/project/:project/modules/:module";
const MODULE_TAB_PATTERN: &str = "/project/:project/modules/:module/:tab";
/// The Delivery view beside a project's Usage (`orchester_workspace::links::delivery_path`).
const DELIVERY_PATTERN: &str = "/project/:project/usage/delivery";
/// A task kept Not shared, by its kept key, on its tab.
const KEPT_TASK_PATTERN: &str = "/project/:project/tasks/kept/:key/:tab";
/// A task's path without its tab, which reads as its Summary.
const TASK_SUMMARY_PATTERN: &str = "/project/:project/tasks/:key";
/// All projects' *Usage* tab, a path of its own under the home screen.
const EVERY_PROJECT_USAGE_PATTERN: &str = "/usage";

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
  /// All projects' *Usage* tab: every project's usage for the query (`rHA8G`).
  Usage(Box<UsageQuery>),
  /// Add project, with the folder it starts with, e.g. one Open folder… found no project in.
  AddProject(Option<RouteFolder>),
  /// Join a team, on the way it shows first (`drCQd`, `B914L`).
  JoinTeam(JoinWay),
  /// The first download of a shared project, the last step of the start page (`YN6MO`).
  FirstDownload(ProjectId),
  /// Settings › Automation, where Settings opens.
  Settings,
  /// Settings › Automation asking *Rotate the token?* in place under the access token (`shpsk`); only a person's
  /// answer rotates it.
  RotateToken,
  /// Settings › General (`l838Ty`): the theme, the plan limits' warning threshold, this copy's updates and this
  /// computer.
  General,
  /// Settings › General showing its *Updates* (`wSMNG`), as the status bar's update item opens it.
  Updates(UpdatesShow),
  /// Settings › General showing its *Plan limits*, the warning threshold's one place, as Usage › Plan limits' menu
  /// opens it.
  PlanLimits,
  /// Settings › Notifications (`fQTFi`): what notifies, how, quiet hours and the daily report.
  Notifications,
  /// Settings › Browser (`j3Lre`): agents' browser, its profiles and the Browser window's switch.
  Browser,
  /// Settings › Accounts, the list or the page of the account it shows with its panel.
  Accounts(AccountsView),
  /// Settings › Orchester Cloud, or the page of the team server it shows, or *Connect a server*.
  Orchester(OrchesterView),
  /// The invite landing (`xeFRB`): which local checkout to link to the project an invite was to.
  Invite(JoinTarget),
  Project(ProjectId, ProjectPage),
}

impl Route {
  /// A pipeline's list, creation page and editor belong to one sidebar screen, with distinct paths.
  pub fn pattern(&self) -> String {
    match self {
      Self::Project(_, ProjectPage::NewPipeline | ProjectPage::NewEmptyPipeline | ProjectPage::CopyPipeline(_)) => {
        NEW_PIPELINE_PATTERN.to_owned()
      }
      Self::Project(_, ProjectPage::Pipeline(..) | ProjectPage::PipelineVersion(..)) => PIPELINE_PATTERN.to_owned(),
      Self::Project(_, ProjectPage::Agent(..) | ProjectPage::NewAgent(_)) => AGENT_PATTERN.to_owned(),
      Self::Project(_, ProjectPage::Files(view)) if view.file().is_some() => FILE_PATTERN.to_owned(),
      Self::Project(_, ProjectPage::PullRequest(..) | ProjectPage::PullRequestFeedback(..)) => {
        PULL_REQUEST_PATTERN.to_owned()
      }
      Self::Project(_, ProjectPage::NewModule | ProjectPage::Module(_, ModuleTab::Summary)) => {
        MODULE_PATTERN.to_owned()
      }
      Self::Project(_, ProjectPage::Module(..) | ProjectPage::EditModule(_)) => MODULE_TAB_PATTERN.to_owned(),
      Self::Project(_, ProjectPage::Delivery(_)) => DELIVERY_PATTERN.to_owned(),
      Self::Invite(_) => INVITE_PATTERN.to_owned(),
      Self::Project(_, ProjectPage::KeptTask(..)) => KEPT_TASK_PATTERN.to_owned(),
      Self::Usage(_) => EVERY_PROJECT_USAGE_PATTERN.to_owned(),
      _ => self.screen().pattern(),
    }
  }
  pub const fn screen(&self) -> Screen {
    match self {
      Self::AllProjects | Self::Usage(_) => Screen::AllProjects,
      Self::AddProject(_) => Screen::AddProject,
      Self::JoinTeam(_) => Screen::JoinTeam,
      Self::FirstDownload(_) => Screen::FirstDownload,
      Self::Settings
      | Self::RotateToken
      | Self::General
      | Self::Updates(_)
      | Self::PlanLimits
      | Self::Notifications
      | Self::Browser
      | Self::Accounts(_)
      | Self::Orchester(_)
      | Self::Invite(_) => Screen::Settings,
      Self::Project(_, page) => page.screen(),
    }
  }

  /// Whether the route is the window's home, which a tab shows by its icon alone, unless it is the active tab at All
  /// projects (`jMVvA` in 09 `i5G19A`): All projects, and Add project and the start page's steps opened from it
  /// (`W5PIR`, `kkGWj`).
  pub const fn is_home(&self) -> bool {
    matches!(
      self,
      Self::AllProjects | Self::Usage(_) | Self::AddProject(_) | Self::JoinTeam(_) | Self::FirstDownload(_)
    )
  }

  pub const fn project(&self) -> Option<ProjectId> {
    match self {
      Self::Project(project, _) => Some(*project),
      Self::AllProjects
      | Self::Usage(_)
      | Self::AddProject(_)
      | Self::JoinTeam(_)
      | Self::FirstDownload(_)
      | Self::Settings
      | Self::RotateToken
      | Self::General
      | Self::Updates(_)
      | Self::PlanLimits
      | Self::Notifications
      | Self::Browser
      | Self::Accounts(_)
      | Self::Orchester(_)
      | Self::Invite(_) => None,
    }
  }

  /// The screen the sidebar shows as selected: a task and its forms belong to Tasks, Connections and Secrets with
  /// their pages to Project settings, whose tabs they are.
  pub const fn nav_screen(&self) -> Screen {
    match self.screen() {
      Screen::Task | Screen::NewTask | Screen::EditTask => Screen::Tasks,
      Screen::Connections | Screen::NewConnection | Screen::EditConnection | Screen::Secrets => Screen::ProjectSettings,
      Screen::OrchestratorSettings => Screen::Orchestrator,
      screen => screen,
    }
  }

  pub fn path(&self) -> String {
    match self {
      Self::AllProjects => "/".to_owned(),
      Self::Usage(query) => usage::everywhere_path(query),
      Self::AddProject(folder) => with_query(
        format!("/{ADD_PROJECT}"),
        &[(FOLDER, folder.as_ref().map(RouteFolder::as_str))],
      ),
      Self::JoinTeam(way) => way.path(),
      Self::FirstDownload(project) => with_query(
        format!("/{FIRST_DOWNLOAD}"),
        &[(PROJECT, Some(project.to_string().as_str()))],
      ),
      Self::Settings => format!("/{SETTINGS}"),
      Self::RotateToken => with_query(format!("/{SETTINGS}"), &[(ASK, Some(ROTATE_TOKEN))]),
      Self::General => with_query(format!("/{SETTINGS}"), &[(TAB, Some(GENERAL))]),
      Self::Updates(show) => with_query(
        format!("/{SETTINGS}"),
        &[(TAB, Some(GENERAL)), (updates::SHOW, Some(show.name()))],
      ),
      Self::PlanLimits => with_query(
        format!("/{SETTINGS}"),
        &[(TAB, Some(GENERAL)), (updates::SHOW, Some(updates::PLAN_LIMITS))],
      ),
      Self::Notifications => with_query(format!("/{SETTINGS}"), &[(TAB, Some(NOTIFICATIONS))]),
      Self::Browser => with_query(format!("/{SETTINGS}"), &[(TAB, Some(BROWSER))]),
      Self::Accounts(view) => view.path(format!("/{SETTINGS}"), (TAB, ACCOUNTS)),
      Self::Orchester(view) => view.path(format!("/{SETTINGS}"), (TAB, ORCHESTER)),
      Self::Invite(target) => {
        let [server, project] = target.params();
        with_query(
          format!("/{SETTINGS}/{INVITE}"),
          &[(server.0, Some(&server.1)), (project.0, Some(&project.1))],
        )
      }
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
      [usage::SEGMENT] => Self::Usage(Box::new(usage::take_everywhere(&mut params)?)),
      [ADD_PROJECT] => Self::AddProject(params.take(FOLDER)?.map(RouteFolder)),
      [join_team::SEGMENT] => Self::JoinTeam(JoinWay::take(&mut params)?),
      [FIRST_DOWNLOAD] => Self::FirstDownload(params.take_parsed(PROJECT)?.ok_or(RouteError::NoScreen)?),
      [SETTINGS] => Self::settings(&mut params)?,
      [SETTINGS, INVITE] => Self::Invite(JoinTarget::take(&mut params)?.ok_or(RouteError::NoScreen)?),
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
  /// Settings at the tab its query names: Automation (asking *Rotate the token?* with `ask`), General (with what it
  /// shows), Notifications, Accounts with the account it shows, Browser, or Orchester Cloud.
  fn settings(params: &mut Params) -> Result<Self, RouteError> {
    match params.take(TAB)?.as_deref() {
      None => match params.take(ASK)?.as_deref() {
        None => Ok(Self::Settings),
        Some(ROTATE_TOKEN) => Ok(Self::RotateToken),
        Some(other) => Err(RouteError::InvalidParam {
          param: ASK.to_owned(),
          value: other.to_owned(),
        }),
      },
      Some(GENERAL) => match params.take(updates::SHOW)? {
        None => Ok(Self::General),
        Some(show) if show == updates::PLAN_LIMITS => Ok(Self::PlanLimits),
        Some(show) => UpdatesShow::named(show).map(Self::Updates),
      },
      Some(NOTIFICATIONS) => Ok(Self::Notifications),
      Some(BROWSER) => Ok(Self::Browser),
      Some(ACCOUNTS) => AccountsView::take(params).map(Self::Accounts),
      Some(ORCHESTER) => OrchesterView::take(params).map(Self::Orchester),
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
