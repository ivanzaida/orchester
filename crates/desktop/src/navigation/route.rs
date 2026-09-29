//! Where the window can be, as typed routes and as the router paths they stand for. A path is
//! the only state history keeps, so everything a user can go back to (the open task, the
//! selected profile, a tab) is part of it. Paths are canonical: a default (the first profile,
//! the All tab, the General tab) is left out, so showing it again adds no entry. Typed text such
//! as a search is not part of a path: going back does not undo typing. A task's Changes tab opened
//! on a file keeps the file in the `file` query, which the tab reads itself (see
//! [`changes_path`]), so the typed routes stay plain values.

use std::{fmt::Write as _, num::NonZeroU32};

use lurq::app::ctx::Ctx;
use orchester_core::{
  id::{ConnectionId, ProjectId, ResourceId, SecretId, TaskId},
  resource::VersionNumber,
};

use crate::tasks::{list::TaskTab, overview::OverviewTab};

const WELCOME: &str = "welcome";
const SETUP: &str = "setup";
const ALL_PROJECTS: &str = "projects";
const PROJECT: &str = "project";
const TASKS: &str = "tasks";
const NEW: &str = "new";
const EDIT: &str = "edit";
const RETRY: &str = "retry";
const PULL_REQUESTS: &str = "pull-requests";
const PIPELINES: &str = "pipelines";
const INSTRUCTIONS: &str = "instructions";
const AGENT_ROLES: &str = "agent-roles";
const PIPELINE_OVERRIDES: &str = "pipeline-overrides";
const STEP_OVERRIDES: &str = "step-overrides";
const SKILLS: &str = "skills";
const AGENTS: &str = "agents";
const TOOLS: &str = "tools";
const SECRETS: &str = "secrets";
const CONNECTIONS: &str = "connections";
const SETTINGS: &str = "settings";
const LIMITS: &str = "limits";
const ENVIRONMENT: &str = "environment";
const TAB_QUERY: &str = "tab";
const AGENT_PROVIDERS: &str = "agent-providers";
const GITHUB: &str = "github";
const MCP_SERVERS: &str = "mcp-servers";
const DRAFTS: &str = "drafts";
const RUNNING: &str = "running";
const NEEDS_YOU: &str = "needs-you";
const QUEUED: &str = "queued";
const ACTIVITY: &str = "activity";
const CHANGES: &str = "changes";
const CONVERSATION: &str = "conversation";
const FILE_QUERY: &str = "file";
const FROM_QUERY: &str = "from";
const WORKTREE: &str = "worktree";
const VERSION_QUERY: &str = "version";
const COMPARE_QUERY: &str = "compare";
const DRAFT: &str = "draft";

/// A screen of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
  /// Before the start screen is known, and the way back to it.
  Start,
  /// First launch (`ZBoBm`).
  Welcome,
  /// Project & repository setup (`W6uXiL`).
  Setup,
  /// All projects (01.06 `Z7IZZ`): the projects of the workspace, to open, add or remove.
  AllProjects,
  /// A screen of an open project.
  Project(ProjectId, Page),
}

/// A screen inside the project shell (`zo9Py` beside it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
  /// `Twtqc`, on a tab of its task list.
  Overview(OverviewTab),
  /// `J9467`, on a tab.
  Tasks(TaskTab),
  /// `l2YA4`.
  NewTask,
  /// One task and its latest run, on a tab (`RD4pc`, `SfXU1`, `vpgrJ`).
  Task(TaskId, TaskPageTab),
  /// A draft task in the New task form, to change it (`E1o77O`).
  EditTask(TaskId),
  /// A queued task with no run, reopened in the locked form to retry its first run.
  RetryTask(TaskId),
  /// `wGtxT`.
  PullRequests,
  /// One pull request of the linked repository, by number (`Rrmhf`).
  PullRequest(u64),
  /// The library (`LRMQW`).
  Pipelines,
  /// One pipeline in its editor (`Q21HOA`), on its draft or a published version.
  Pipeline(ResourceId, PipelineView),
  /// Instructions of one scope, and the one being edited (`yRKeF`); the first of the scope
  /// without one.
  Instructions(InstructionsTab, Option<ResourceId>),
  /// The skills library (`qO1Az`, empty `YRaLK`) or one skill in its editor (`ecJtg`).
  Skills(Option<ResourceId>),
  /// Agent profiles and the one being edited (`WPmtS`).
  Agents(AgentSelection),
  /// The tool library (`ZFwWl`) or one tool in its editor (`YEWFo`).
  Tools(ToolSelection),
  /// Project secrets and the one whose details are shown (`R99lt`); without one, the first
  /// listed.
  Secrets(Option<SecretId>),
  /// Connections, the tab and the connection whose details are shown (`a4Lqb`).
  Connections(ConnectionTab, Option<ConnectionId>),
  /// `QV1af`, `c7e729`.
  Settings(SettingsTab),
}

/// What the pipeline editor shows: the draft to edit, or a published version read-only beside
/// the version history, alone or compared with the draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineView {
  Draft,
  Version(VersionNumber),
  Compare(VersionNumber),
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

/// What the Tools screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolSelection {
  Library,
  Tool(ResourceId),
  /// A command tool being written that is not saved yet.
  New,
}

/// The tabs of a task's page that show something (`X7jYe`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskPageTab {
  Summary,
  Conversation,
  Changes,
  Activity,
}

/// The scope tabs of Instructions (`GfEHL`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstructionsTab {
  ProjectRules,
  AgentRoles,
  PipelineOverrides,
  StepOverrides,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionTab {
  All,
  AgentProviders,
  GitHub,
  McpServers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
  General,
  Limits,
  Environment,
}

/// The sidebar item a page belongs to: Tasks stays selected on a task or the new task form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarItem {
  Overview,
  Tasks,
  PullRequests,
  Pipelines,
  Instructions,
  Skills,
  Agents,
  Tools,
  Secrets,
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
      Self::Start | Self::Welcome | Self::Setup | Self::AllProjects => None,
    }
  }

  pub fn parse(path: &str) -> Option<Self> {
    let (path, query) = path.split_once('?').unwrap_or((path, ""));
    let segments: Vec<&str> = path.split('/').filter(|segment| !segment.is_empty()).collect();
    match segments.as_slice() {
      [] => Some(Self::Start),
      [WELCOME] => Some(Self::Welcome),
      [SETUP] => Some(Self::Setup),
      [ALL_PROJECTS] => Some(Self::AllProjects),
      [PROJECT, project, rest @ ..] => Some(Self::Project(project.parse().ok()?, Page::parse(rest, query)?)),
      _ => None,
    }
  }

  /// The route of `path`, or of the longest leading part of it that names one: a task that is
  /// not an id leads to the task list, an unknown settings tab to the settings.
  pub fn nearest(path: &str) -> Option<Self> {
    let path = path.split('?').next().unwrap_or(path).trim_end_matches('/');
    let mut candidate = path;
    loop {
      if let Some(route) = Self::parse(candidate) {
        return Some(route);
      }
      candidate = &candidate[..candidate.rfind('/')?];
    }
  }

  pub fn path(&self) -> String {
    match self {
      Self::Start => "/".to_owned(),
      Self::Welcome => format!("/{WELCOME}"),
      Self::Setup => format!("/{SETUP}"),
      Self::AllProjects => format!("/{ALL_PROJECTS}"),
      Self::Project(project, page) => page.path(*project),
    }
  }
}

impl Page {
  /// The overview on the All tab.
  pub const OVERVIEW: Self = Self::Overview(OverviewTab::All);
  /// The task list on the All tab.
  pub const TASKS: Self = Self::Tasks(TaskTab::All);
  /// Secrets, with the first secret's details.
  pub const SECRETS: Self = Self::Secrets(None);
  /// Connections on the All tab, with the first connection's details.
  pub const CONNECTIONS: Self = Self::Connections(ConnectionTab::All, None);

  /// `task` on its Summary tab.
  pub const fn task(task: TaskId) -> Self {
    Self::Task(task, TaskPageTab::Summary)
  }

  /// Whether both pages show the same screen, whatever tab it is on.
  pub fn same_screen(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Overview(_), Self::Overview(_)) | (Self::Tasks(_), Self::Tasks(_)) => true,
      (Self::Task(task, _), Self::Task(other, _)) => task == other,
      (Self::Pipeline(pipeline, _), Self::Pipeline(other, _)) => pipeline == other,
      _ => self == other,
    }
  }

  fn parse(segments: &[&str], query: &str) -> Option<Self> {
    let tab = tab_query(query);
    Some(match segments {
      [] => Self::Overview(overview_tab(tab)),
      [TASKS] => Self::Tasks(task_tab(tab)),
      [TASKS, NEW] => Self::NewTask,
      [TASKS, task] => Self::Task(task.parse().ok()?, task_page_tab(tab)),
      [TASKS, task, EDIT] => Self::EditTask(task.parse().ok()?),
      [TASKS, task, RETRY] => Self::RetryTask(task.parse().ok()?),
      [PULL_REQUESTS] => Self::PullRequests,
      [PULL_REQUESTS, number] => Self::PullRequest(number.parse().ok()?),
      [PIPELINES] => Self::Pipelines,
      [PIPELINES, pipeline] => Self::Pipeline(pipeline.parse().ok()?, PipelineView::named(query)),
      [INSTRUCTIONS] => Self::Instructions(InstructionsTab::named(tab), None),
      [INSTRUCTIONS, instruction] => Self::Instructions(InstructionsTab::named(tab), Some(instruction.parse().ok()?)),
      [SKILLS] => Self::Skills(None),
      [SKILLS, skill] => Self::Skills(Some(skill.parse().ok()?)),
      [AGENTS] => Self::Agents(AgentSelection::First),
      [AGENTS, NEW] => Self::Agents(AgentSelection::New),
      [AGENTS, profile] => Self::Agents(AgentSelection::Profile(profile.parse().ok()?)),
      [TOOLS] => Self::Tools(ToolSelection::Library),
      [TOOLS, NEW] => Self::Tools(ToolSelection::New),
      [TOOLS, tool] => Self::Tools(ToolSelection::Tool(tool.parse().ok()?)),
      [SECRETS] => Self::SECRETS,
      [SECRETS, secret] => Self::Secrets(Some(secret.parse().ok()?)),
      [CONNECTIONS] => Self::Connections(ConnectionTab::named(tab), None),
      [CONNECTIONS, connection] => Self::Connections(ConnectionTab::named(tab), Some(connection.parse().ok()?)),
      [SETTINGS] => Self::Settings(SettingsTab::General),
      [SETTINGS, LIMITS] => Self::Settings(SettingsTab::Limits),
      [SETTINGS, ENVIRONMENT] => Self::Settings(SettingsTab::Environment),
      _ => return None,
    })
  }

  /// The path of this page in `project`.
  pub fn path(&self, project: ProjectId) -> String {
    let root = format!("/{PROJECT}/{project}");
    match self {
      Self::Overview(tab) => with_tab(root, overview_query(*tab)),
      Self::Tasks(tab) => with_tab(format!("{root}/{TASKS}"), task_query(*tab)),
      Self::NewTask => format!("{root}/{TASKS}/{NEW}"),
      Self::Task(task, tab) => with_tab(format!("{root}/{TASKS}/{task}"), task_page_query(*tab)),
      Self::EditTask(task) => format!("{root}/{TASKS}/{task}/{EDIT}"),
      Self::RetryTask(task) => format!("{root}/{TASKS}/{task}/{RETRY}"),
      Self::PullRequests => format!("{root}/{PULL_REQUESTS}"),
      Self::PullRequest(number) => format!("{root}/{PULL_REQUESTS}/{number}"),
      Self::Pipelines => format!("{root}/{PIPELINES}"),
      Self::Pipeline(pipeline, view) => format!("{root}/{PIPELINES}/{pipeline}{}", view.query()),
      Self::Instructions(tab, selected) => {
        let selected = selected.map_or_else(String::new, |instruction| format!("/{instruction}"));
        with_tab(format!("{root}/{INSTRUCTIONS}{selected}"), tab.query())
      }
      Self::Skills(None) => format!("{root}/{SKILLS}"),
      Self::Skills(Some(skill)) => format!("{root}/{SKILLS}/{skill}"),
      Self::Agents(AgentSelection::First) => format!("{root}/{AGENTS}"),
      Self::Agents(AgentSelection::New) => format!("{root}/{AGENTS}/{NEW}"),
      Self::Agents(AgentSelection::Profile(profile)) => format!("{root}/{AGENTS}/{profile}"),
      Self::Tools(ToolSelection::Library) => format!("{root}/{TOOLS}"),
      Self::Tools(ToolSelection::New) => format!("{root}/{TOOLS}/{NEW}"),
      Self::Tools(ToolSelection::Tool(tool)) => format!("{root}/{TOOLS}/{tool}"),
      Self::Secrets(selected) => {
        let selected = selected.map_or_else(String::new, |secret| format!("/{secret}"));
        format!("{root}/{SECRETS}{selected}")
      }
      Self::Connections(tab, selected) => {
        let selected = selected.map_or_else(String::new, |connection| format!("/{connection}"));
        with_tab(format!("{root}/{CONNECTIONS}{selected}"), tab.query())
      }
      Self::Settings(SettingsTab::General) => format!("{root}/{SETTINGS}"),
      Self::Settings(SettingsTab::Limits) => format!("{root}/{SETTINGS}/{LIMITS}"),
      Self::Settings(SettingsTab::Environment) => format!("{root}/{SETTINGS}/{ENVIRONMENT}"),
    }
  }

  pub const fn sidebar_item(&self) -> SidebarItem {
    match self {
      Self::Overview(_) => SidebarItem::Overview,
      Self::Tasks(_) | Self::NewTask | Self::Task(..) | Self::EditTask(_) | Self::RetryTask(_) => SidebarItem::Tasks,
      Self::PullRequests | Self::PullRequest(_) => SidebarItem::PullRequests,
      Self::Pipelines | Self::Pipeline(..) => SidebarItem::Pipelines,
      Self::Instructions(..) => SidebarItem::Instructions,
      Self::Skills(_) => SidebarItem::Skills,
      Self::Agents(_) => SidebarItem::Agents,
      Self::Tools(_) => SidebarItem::Tools,
      Self::Secrets(_) => SidebarItem::Secrets,
      Self::Connections(..) => SidebarItem::Connections,
      Self::Settings(_) => SidebarItem::Settings,
    }
  }
}

/// The tab a query names, if it names one.
fn tab_query(query: &str) -> Option<&str> {
  query_value(query, TAB_QUERY)
}

/// The value of `name` in `query`, if it has one.
fn query_value<'a>(query: &'a str, name: &str) -> Option<&'a str> {
  query
    .split('&')
    .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
}

/// `path` on `tab`; the default tab (`None`) is left out.
fn with_tab(path: String, tab: Option<&str>) -> String {
  match tab {
    Some(tab) => format!("{path}?{TAB_QUERY}={tab}"),
    None => path,
  }
}

/// No tab, or one that does not exist, is the first tab of a screen.
fn overview_tab(tab: Option<&str>) -> OverviewTab {
  match tab {
    Some(RUNNING) => OverviewTab::Running,
    Some(NEEDS_YOU) => OverviewTab::NeedsYou,
    Some(QUEUED) => OverviewTab::Queued,
    _ => OverviewTab::All,
  }
}

const fn overview_query(tab: OverviewTab) -> Option<&'static str> {
  match tab {
    OverviewTab::All => None,
    OverviewTab::Running => Some(RUNNING),
    OverviewTab::NeedsYou => Some(NEEDS_YOU),
    OverviewTab::Queued => Some(QUEUED),
  }
}

/// Only the tabs that are available have a path; the others cannot be opened yet.
fn task_tab(tab: Option<&str>) -> TaskTab {
  match tab {
    Some(DRAFTS) => TaskTab::Drafts,
    _ => TaskTab::All,
  }
}

const fn task_query(tab: TaskTab) -> Option<&'static str> {
  match tab {
    TaskTab::Drafts => Some(DRAFTS),
    TaskTab::All | TaskTab::Queue | TaskTab::NeedsReview | TaskTab::Done => None,
  }
}

fn task_page_tab(tab: Option<&str>) -> TaskPageTab {
  match tab {
    Some(CONVERSATION) => TaskPageTab::Conversation,
    Some(CHANGES) => TaskPageTab::Changes,
    Some(ACTIVITY) => TaskPageTab::Activity,
    _ => TaskPageTab::Summary,
  }
}

const fn task_page_query(tab: TaskPageTab) -> Option<&'static str> {
  match tab {
    TaskPageTab::Summary => None,
    TaskPageTab::Conversation => Some(CONVERSATION),
    TaskPageTab::Changes => Some(CHANGES),
    TaskPageTab::Activity => Some(ACTIVITY),
  }
}

impl PipelineView {
  /// The view a query names. Without a version, or with one that is not a version number, it is
  /// the draft, as an unknown tab is a screen's first tab.
  fn named(query: &str) -> Self {
    let Some(version) = query_value(query, VERSION_QUERY).and_then(|version| version.parse::<NonZeroU32>().ok()) else {
      return Self::Draft;
    };
    let version = VersionNumber::new(version);
    match query_value(query, COMPARE_QUERY) {
      Some(DRAFT) => Self::Compare(version),
      _ => Self::Version(version),
    }
  }

  fn query(self) -> String {
    match self {
      Self::Draft => String::new(),
      Self::Version(version) => format!("?{VERSION_QUERY}={}", version.get()),
      Self::Compare(version) => format!("?{VERSION_QUERY}={}&{COMPARE_QUERY}={DRAFT}", version.get()),
    }
  }

  /// The published version shown, if one is.
  pub const fn version(self) -> Option<VersionNumber> {
    match self {
      Self::Draft => None,
      Self::Version(version) | Self::Compare(version) => Some(version),
    }
  }
}

impl InstructionsTab {
  fn named(tab: Option<&str>) -> Self {
    match tab {
      Some(AGENT_ROLES) => Self::AgentRoles,
      Some(PIPELINE_OVERRIDES) => Self::PipelineOverrides,
      Some(STEP_OVERRIDES) => Self::StepOverrides,
      _ => Self::ProjectRules,
    }
  }

  const fn query(self) -> Option<&'static str> {
    match self {
      Self::ProjectRules => None,
      Self::AgentRoles => Some(AGENT_ROLES),
      Self::PipelineOverrides => Some(PIPELINE_OVERRIDES),
      Self::StepOverrides => Some(STEP_OVERRIDES),
    }
  }
}

/// A changed file a task's Changes tab shows: its repository path (`/`-separated), and whether it
/// is the version in the run's worktree that is not checkpointed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
  pub path: String,
  pub worktree: bool,
}

/// The path of `task`'s Changes tab in `project`, on `file`.
pub fn changes_path(project: ProjectId, task: TaskId, file: &ChangedFile) -> String {
  let tab = Page::Task(task, TaskPageTab::Changes).path(project);
  let from = if file.worktree {
    format!("&{FROM_QUERY}={WORKTREE}")
  } else {
    String::new()
  };
  format!("{tab}&{FILE_QUERY}={}{from}", encode(&file.path))
}

/// The changed file the queries of `path` name, if they name one.
pub fn changed_file(path: &str) -> Option<ChangedFile> {
  let (_, query) = path.split_once('?')?;
  let value = |name: &str| {
    query
      .split('&')
      .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
  };
  let path = decode(value(FILE_QUERY)?).filter(|file| !file.is_empty())?;
  Some(ChangedFile {
    path,
    worktree: value(FROM_QUERY) == Some(WORKTREE),
  })
}

/// `value` with every byte outside the unreserved characters and `/` percent-encoded, so a path
/// with spaces, `&`, `?`, `#` or non-ASCII names stays one query value.
fn encode(value: &str) -> String {
  let mut encoded = String::with_capacity(value.len());
  for byte in value.bytes() {
    if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
      encoded.push(char::from(byte));
    } else {
      write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail");
    }
  }
  encoded
}

/// [`encode`] undone; `None` for a malformed escape or bytes that are not UTF-8.
fn decode(value: &str) -> Option<String> {
  let mut bytes = Vec::with_capacity(value.len());
  let mut rest = value.bytes();
  while let Some(byte) = rest.next() {
    if byte != b'%' {
      bytes.push(byte);
      continue;
    }
    let digits = [rest.next()?, rest.next()?];
    let hex = std::str::from_utf8(&digits).ok()?;
    bytes.push(u8::from_str_radix(hex, 16).ok()?);
  }
  String::from_utf8(bytes).ok()
}

impl ConnectionTab {
  fn named(tab: Option<&str>) -> Self {
    match tab {
      Some(AGENT_PROVIDERS) => Self::AgentProviders,
      Some(GITHUB) => Self::GitHub,
      Some(MCP_SERVERS) => Self::McpServers,
      _ => Self::All,
    }
  }

  const fn query(self) -> Option<&'static str> {
    match self {
      Self::All => None,
      Self::AgentProviders => Some(AGENT_PROVIDERS),
      Self::GitHub => Some(GITHUB),
      Self::McpServers => Some(MCP_SERVERS),
    }
  }
}
