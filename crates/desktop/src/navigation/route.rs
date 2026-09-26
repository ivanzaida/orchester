//! Where the window can be, as typed routes and as the router paths they stand for. A path is
//! the only state history keeps, so everything a user can go back to (the open task, the
//! selected profile, a settings tab) is part of it. Paths are canonical: a default (the first
//! profile, the All tab, the General tab) is left out, so showing it again adds no entry.

use lurq::app::ctx::Ctx;
use orchester_core::id::{ConnectionId, ProjectId, ResourceId, TaskId};

const WELCOME: &str = "welcome";
const SETUP: &str = "setup";
const UNAVAILABLE: &str = "unavailable";
const PROJECT: &str = "project";
const TASKS: &str = "tasks";
const NEW: &str = "new";
const PULL_REQUESTS: &str = "pull-requests";
const PIPELINES: &str = "pipelines";
const AGENTS: &str = "agents";
const CONNECTIONS: &str = "connections";
const SETTINGS: &str = "settings";
const LIMITS: &str = "limits";
const TAB_QUERY: &str = "tab";
const AGENT_PROVIDERS: &str = "agent-providers";
const GITHUB: &str = "github";

/// A screen of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
  /// Before the start screen is known, and the way back to it.
  Start,
  /// First launch (`ZBoBm`).
  Welcome,
  /// Project & repository setup (`W6uXiL`).
  Setup,
  /// The most recently used project could not be opened.
  Unavailable,
  /// A screen of an open project.
  Project(ProjectId, Page),
}

/// A screen inside the project shell (`zo9Py` beside it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
  /// `Twtqc`.
  Overview,
  /// `J9467`.
  Tasks,
  /// `l2YA4`.
  NewTask,
  /// One task and its latest run (`RD4pc`, `SfXU1`).
  Task(TaskId),
  /// `wGtxT`.
  PullRequests,
  /// One pull request of the linked repository, by number (`Rrmhf`).
  PullRequest(u64),
  /// The library (`LRMQW`).
  Pipelines,
  /// One pipeline in its editor (`Q21HOA`).
  Pipeline(ResourceId),
  /// Agent profiles and the one being edited (`WPmtS`).
  Agents(AgentSelection),
  /// Connections, the tab and the connection whose details are shown (`a4Lqb`).
  Connections(ConnectionTab, Option<ConnectionId>),
  /// `QV1af`, `c7e729`.
  Settings(SettingsTab),
}

/// The profile the Agents screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentSelection {
  /// The first profile, if there is one.
  First,
  Profile(ResourceId),
  /// A profile being written that is not saved yet.
  New,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionTab {
  All,
  AgentProviders,
  GitHub,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
  General,
  Limits,
}

/// The sidebar item a page belongs to: Tasks stays selected on a task or the new task form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarItem {
  Overview,
  Tasks,
  PullRequests,
  Pipelines,
  Agents,
  Connections,
  Settings,
}

impl Route {
  /// The route the router shows, read reactively: a component reading it renders again when
  /// the location changes. `None` for a path no route stands for.
  pub fn current(ctx: &mut Ctx) -> Option<Self> {
    Self::parse(&ctx.route_path())
  }

  /// The page of the open project the router shows, read as [`Route::current`] is.
  pub fn current_page(ctx: &mut Ctx) -> Option<Page> {
    match Self::current(ctx)? {
      Self::Project(_, page) => Some(page),
      Self::Start | Self::Welcome | Self::Setup | Self::Unavailable => None,
    }
  }

  pub fn parse(path: &str) -> Option<Self> {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
    match segments.as_slice() {
      [] => Some(Self::Start),
      [WELCOME] => Some(Self::Welcome),
      [SETUP] => Some(Self::Setup),
      [UNAVAILABLE] => Some(Self::Unavailable),
      [PROJECT, project, rest @ ..] => Some(Self::Project(project.parse().ok()?, Page::parse(rest, query)?)),
      _ => None,
    }
  }

  pub fn path(&self) -> String {
    match self {
      Self::Start => "/".to_owned(),
      Self::Welcome => format!("/{WELCOME}"),
      Self::Setup => format!("/{SETUP}"),
      Self::Unavailable => format!("/{UNAVAILABLE}"),
      Self::Project(project, page) => page.path(*project),
    }
  }
}

impl Page {
  /// Connections on the All tab, with the first connection's details.
  pub const CONNECTIONS: Self = Self::Connections(ConnectionTab::All, None);

  fn parse(segments: &[&str], query: &str) -> Option<Self> {
    Some(match segments {
      [] => Self::Overview,
      [TASKS] => Self::Tasks,
      [TASKS, NEW] => Self::NewTask,
      [TASKS, task] => Self::Task(task.parse().ok()?),
      [PULL_REQUESTS] => Self::PullRequests,
      [PULL_REQUESTS, number] => Self::PullRequest(number.parse().ok()?),
      [PIPELINES] => Self::Pipelines,
      [PIPELINES, pipeline] => Self::Pipeline(pipeline.parse().ok()?),
      [AGENTS] => Self::Agents(AgentSelection::First),
      [AGENTS, NEW] => Self::Agents(AgentSelection::New),
      [AGENTS, profile] => Self::Agents(AgentSelection::Profile(profile.parse().ok()?)),
      [CONNECTIONS] => Self::Connections(ConnectionTab::from_query(query)?, None),
      [CONNECTIONS, connection] => Self::Connections(ConnectionTab::from_query(query)?, Some(connection.parse().ok()?)),
      [SETTINGS] => Self::Settings(SettingsTab::General),
      [SETTINGS, LIMITS] => Self::Settings(SettingsTab::Limits),
      _ => return None,
    })
  }

  /// The path of this page in `project`.
  pub fn path(&self, project: ProjectId) -> String {
    let root = format!("/{PROJECT}/{project}");
    match self {
      Self::Overview => root,
      Self::Tasks => format!("{root}/{TASKS}"),
      Self::NewTask => format!("{root}/{TASKS}/{NEW}"),
      Self::Task(task) => format!("{root}/{TASKS}/{task}"),
      Self::PullRequests => format!("{root}/{PULL_REQUESTS}"),
      Self::PullRequest(number) => format!("{root}/{PULL_REQUESTS}/{number}"),
      Self::Pipelines => format!("{root}/{PIPELINES}"),
      Self::Pipeline(pipeline) => format!("{root}/{PIPELINES}/{pipeline}"),
      Self::Agents(AgentSelection::First) => format!("{root}/{AGENTS}"),
      Self::Agents(AgentSelection::New) => format!("{root}/{AGENTS}/{NEW}"),
      Self::Agents(AgentSelection::Profile(profile)) => format!("{root}/{AGENTS}/{profile}"),
      Self::Connections(tab, selected) => {
        let selected = selected.map_or_else(String::new, |connection| format!("/{connection}"));
        let tab = tab
          .query()
          .map_or_else(String::new, |tab| format!("?{TAB_QUERY}={tab}"));
        format!("{root}/{CONNECTIONS}{selected}{tab}")
      }
      Self::Settings(SettingsTab::General) => format!("{root}/{SETTINGS}"),
      Self::Settings(SettingsTab::Limits) => format!("{root}/{SETTINGS}/{LIMITS}"),
    }
  }

  pub const fn sidebar_item(&self) -> SidebarItem {
    match self {
      Self::Overview => SidebarItem::Overview,
      Self::Tasks | Self::NewTask | Self::Task(_) => SidebarItem::Tasks,
      Self::PullRequests | Self::PullRequest(_) => SidebarItem::PullRequests,
      Self::Pipelines | Self::Pipeline(_) => SidebarItem::Pipelines,
      Self::Agents(_) => SidebarItem::Agents,
      Self::Connections(..) => SidebarItem::Connections,
      Self::Settings(_) => SidebarItem::Settings,
    }
  }
}

impl ConnectionTab {
  /// The tab a query names; no tab is All, an unknown one no page.
  fn from_query(query: &str) -> Option<Self> {
    let tab = query
      .split('&')
      .find_map(|pair| pair.strip_prefix(TAB_QUERY)?.strip_prefix('='));
    match tab {
      None => Some(Self::All),
      Some(AGENT_PROVIDERS) => Some(Self::AgentProviders),
      Some(GITHUB) => Some(Self::GitHub),
      Some(_) => None,
    }
  }

  const fn query(self) -> Option<&'static str> {
    match self {
      Self::All => None,
      Self::AgentProviders => Some(AGENT_PROVIDERS),
      Self::GitHub => Some(GITHUB),
    }
  }
}
