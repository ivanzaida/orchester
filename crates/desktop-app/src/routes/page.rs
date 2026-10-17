use std::num::NonZeroU64;

use orchester_core::{
  id::{ConnectionId, ResourceId},
  project::TaskKey,
};
use orchester_workspace::{links::USAGE, pull_requests::list::PullRequestTab, usage::UsageQuery};

use super::{
  CONNECTIONS, JoinTarget, ConnectionsView, EDIT, FilesView, NEW, NewConnection, ORCHESTRATOR, OrchestratorChat, OverviewTab,
  PULL_REQUESTS, RouteError, SETTINGS, Screen, TAB, TASKS, TaskList, TaskView, ToolTab,
  files::FILES,
  project_screen_path,
  query::{Params, with_query},
};

/// Project settings' Sharing tab, and what it asks once open.
const SHARING: &str = "sharing";
const ASK: &str = "ask";
const MAKE_LOCAL: &str = "make-local";
/// The join review under Sharing.
const JOIN: &str = "join";

/// A screen of one project, with what its path names besides the project.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectPage {
  Overview(OverviewTab),
  Orchestrator(OrchestratorChat),
  OrchestratorSettings,
  Tasks(TaskList),
  NewTask,
  EditTask(TaskKey),
  Task(TaskKey, TaskView),
  MergeQueue,
  /// The Pull requests page on its tab.
  PullRequests(PullRequestTab),
  /// A pull request of the project's repository, by its number.
  PullRequest(NonZeroU64),
  /// The Usage page with its period, grouping and filters (boxed: a query holds many filters).
  Usage(Box<UsageQuery>),
  Pipelines,
  NewPipeline,
  NewEmptyPipeline,
  Pipeline(ResourceId),
  Agents,
  Agent(ResourceId),
  Instructions,
  Memory,
  Skills,
  NewSkill,
  Skill(ResourceId),
  ImportSkills,
  Tools,
  NewTool,
  Tool(ResourceId, ToolTab),
  Secrets,
  Connections(ConnectionsView),
  NewConnection(NewConnection),
  /// Edit MCP server, the server's id.
  EditConnection(ConnectionId),
  Settings,
  /// Project settings on its Sharing tab; `make_local` opens *Make local*'s question there (the task list's
  /// *no longer a member* notice).
  Sharing {
    make_local: bool,
  },
  /// The join review (`AiCwO`) of the project `JoinTarget` names, over Project settings › Sharing.
  Join(JoinTarget),
  /// The code viewer: a file, its version and lines.
  Files(FilesView),
}

impl ProjectPage {
  /// The page of a project screen that names nothing but the project, as it opens: every one but the
  /// screens of a task.
  pub fn of(screen: Screen) -> Option<Self> {
    Some(match screen {
      Screen::AllProjects
      | Screen::AddProject
      | Screen::Settings
      | Screen::EditTask
      | Screen::Task
      | Screen::EditConnection => {
        return None;
      }
      Screen::Overview => Self::Overview(OverviewTab::default()),
      Screen::Orchestrator => Self::Orchestrator(OrchestratorChat::default()),
      Screen::OrchestratorSettings => Self::OrchestratorSettings,
      Screen::Tasks => Self::Tasks(TaskList::default()),
      Screen::NewTask => Self::NewTask,
      Screen::MergeQueue => Self::MergeQueue,
      Screen::PullRequests => Self::PullRequests(PullRequestTab::default()),
      Screen::Usage => Self::Usage(Box::default()),
      Screen::Pipelines => Self::Pipelines,
      Screen::Agents => Self::Agents,
      Screen::Instructions => Self::Instructions,
      Screen::Memory => Self::Memory,
      Screen::Skills => Self::Skills,
      Screen::Tools => Self::Tools,
      Screen::Secrets => Self::Secrets,
      Screen::Connections => Self::Connections(ConnectionsView::default()),
      Screen::NewConnection => Self::NewConnection(NewConnection::default()),
      Screen::ProjectSettings => Self::Settings,
      Screen::Files => Self::Files(FilesView::default()),
    })
  }

  pub const fn screen(&self) -> Screen {
    match self {
      Self::Overview(_) => Screen::Overview,
      Self::Orchestrator(_) => Screen::Orchestrator,
      Self::OrchestratorSettings => Screen::OrchestratorSettings,
      Self::Tasks(_) => Screen::Tasks,
      Self::NewTask => Screen::NewTask,
      Self::EditTask(_) => Screen::EditTask,
      Self::Task(..) => Screen::Task,
      Self::MergeQueue => Screen::MergeQueue,
      Self::PullRequests(_) | Self::PullRequest(_) => Screen::PullRequests,
      Self::Usage(_) => Screen::Usage,
      Self::Pipelines | Self::NewPipeline | Self::NewEmptyPipeline | Self::Pipeline(_) => Screen::Pipelines,
      Self::Agents | Self::Agent(_) => Screen::Agents,
      Self::Instructions => Screen::Instructions,
      Self::Memory => Screen::Memory,
      Self::Skills | Self::NewSkill | Self::Skill(_) | Self::ImportSkills => Screen::Skills,
      Self::Tools | Self::NewTool | Self::Tool(..) => Screen::Tools,
      Self::Secrets => Screen::Secrets,
      Self::Connections(_) => Screen::Connections,
      Self::NewConnection(_) => Screen::NewConnection,
      Self::EditConnection(_) => Screen::EditConnection,
      Self::Settings | Self::Sharing { .. } | Self::Join(_) => Screen::ProjectSettings,
      Self::Files(_) => Screen::Files,
    }
  }

  pub(super) fn path(&self, project_root: &str) -> String {
    let tasks = format!("{project_root}/{TASKS}");
    let connections = format!("{project_root}/{CONNECTIONS}");
    match self {
      Self::Overview(tab) => {
        let tab = (*tab != OverviewTab::default()).then(|| tab.name());
        with_query(project_root.to_owned(), &[(TAB, tab)])
      }
      Self::Orchestrator(chat) => chat.path(format!("{project_root}/{ORCHESTRATOR}")),
      Self::OrchestratorSettings => format!("{project_root}/{ORCHESTRATOR}/{SETTINGS}"),
      Self::Tasks(list) => list.path(tasks),
      Self::NewTask => format!("{tasks}/{NEW}"),
      Self::EditTask(key) => format!("{tasks}/{key}/{EDIT}"),
      Self::Task(key, view) => view.path(&format!("{tasks}/{key}")),
      Self::NewSkill => with_query(format!("{project_root}/skills"), &[("skill", Some("new"))]),
      Self::Skill(id) => with_query(format!("{project_root}/skills"), &[("skill", Some(&id.to_string()))]),
      Self::ImportSkills => with_query(format!("{project_root}/skills"), &[("skill", Some("import"))]),
      Self::NewTool => with_query(format!("{project_root}/tools"), &[("tool", Some("new"))]),
      Self::Tool(id, tab) => with_query(
        format!("{project_root}/tools"),
        &[("tool", Some(&id.to_string())), (TAB, Some(tab.name()))],
      ),
      Self::NewPipeline => format!("{project_root}/pipelines/new"),
      Self::NewEmptyPipeline => format!("{project_root}/pipelines/new?start=empty"),
      Self::Pipeline(id) => format!("{project_root}/pipelines/{id}"),
      Self::Agent(id) => format!("{project_root}/agents/{id}"),
      Self::PullRequests(tab) => {
        let tab = (*tab != PullRequestTab::default()).then(|| tab.name());
        with_query(format!("{project_root}/{PULL_REQUESTS}"), &[(TAB, tab)])
      }
      Self::PullRequest(number) => format!("{project_root}/{PULL_REQUESTS}/{number}"),
      Self::Usage(query) => super::usage::path(project_root, query),
      Self::Connections(view) => view.path(connections),
      Self::NewConnection(kind) => kind.path(format!("{connections}/{NEW}")),
      Self::EditConnection(id) => format!("{connections}/{id}/{EDIT}"),
      Self::Files(view) => view.path(format!("{project_root}/{FILES}")),
      Self::Sharing { make_local } => with_query(
        format!("{project_root}/{SETTINGS}/{SHARING}"),
        &[(ASK, make_local.then_some(MAKE_LOCAL))],
      ),
      Self::Join(target) => {
        let [server, project] = target.params();
        with_query(
          format!("{project_root}/{SETTINGS}/{SHARING}/{JOIN}"),
          &[(server.0, Some(&server.1)), (project.0, Some(&project.1))],
        )
      }
      page => project_screen_path(project_root, page.screen()),
    }
  }

  /// The page the segments after a project's path and the query name.
  pub(super) fn parse(segments: &[&str], params: &mut Params) -> Result<Self, RouteError> {
    match segments {
      [] => Ok(Self::Overview(params.take_or_default(TAB, OverviewTab::from_name)?)),
      [ORCHESTRATOR] => OrchestratorChat::take(params).map(Self::Orchestrator),
      [ORCHESTRATOR, SETTINGS] => Ok(Self::OrchestratorSettings),
      [SETTINGS, SHARING] => match params.take(ASK)?.as_deref() {
        None => Ok(Self::Sharing { make_local: false }),
        Some(MAKE_LOCAL) => Ok(Self::Sharing { make_local: true }),
        Some(value) => Err(RouteError::InvalidParam {
          param: ASK.to_owned(),
          value: value.to_owned(),
        }),
      },
      [SETTINGS, SHARING, JOIN] => JoinTarget::take(params)?.map(Self::Join).ok_or(RouteError::NoScreen),
      [TASKS] => TaskList::take(params).map(Self::Tasks),
      [TASKS, NEW] => Ok(Self::NewTask),
      [TASKS, key, rest @ ..] => {
        let key = task_key(key)?;
        match rest {
          [EDIT] => Ok(Self::EditTask(key)),
          rest => TaskView::parse(rest, params).map(|view| Self::Task(key, view)),
        }
      }
      ["skills"] => match params.take("skill")?.as_deref() {
        None => Ok(Self::Skills),
        Some("new") => Ok(Self::NewSkill),
        Some("import") => Ok(Self::ImportSkills),
        Some(id) => id.parse().map(Self::Skill).map_err(|_| RouteError::NoScreen),
      },
      ["tools"] => match params.take("tool")?.as_deref() {
        None => Ok(Self::Tools),
        Some("new") => Ok(Self::NewTool),
        Some(id) => {
          let id = id.parse().map_err(|_| RouteError::NoScreen)?;
          let tab = params.take_or_default(TAB, ToolTab::from_name)?;
          Ok(Self::Tool(id, tab))
        }
      },
      ["pipelines", NEW] => match params.take("start")?.as_deref() {
        None | Some("starter") => Ok(Self::NewPipeline),
        Some("empty") => Ok(Self::NewEmptyPipeline),
        Some(value) => Err(RouteError::InvalidParam {
          param: "start".to_owned(),
          value: value.to_owned(),
        }),
      },
      ["pipelines", id] => id.parse().map(Self::Pipeline).map_err(|_| RouteError::NoScreen),
      ["agents", id] => id.parse().map(Self::Agent).map_err(|_| RouteError::NoScreen),
      [PULL_REQUESTS] => Ok(Self::PullRequests(
        params.take_or_default(TAB, PullRequestTab::from_name)?,
      )),
      [USAGE] => super::usage::take(params).map(|query| Self::Usage(Box::new(query))),
      [PULL_REQUESTS, number] => number
        .parse::<NonZeroU64>()
        .map(Self::PullRequest)
        .map_err(|_| RouteError::NoScreen),
      [CONNECTIONS] => ConnectionsView::take(params).map(Self::Connections),
      [CONNECTIONS, NEW] => NewConnection::take(params).map(Self::NewConnection),
      [FILES, file @ ..] => FilesView::parse(file, params).map(Self::Files),
      [CONNECTIONS, id, EDIT] => id.parse().map(Self::EditConnection).map_err(|_| RouteError::NoScreen),
      [segment] => Screen::ALL
        .into_iter()
        .find(|screen| screen.segment() == Some(*segment))
        .and_then(Self::of)
        .ok_or(RouteError::NoScreen),
      _ => Err(RouteError::NoScreen),
    }
  }
}

fn task_key(value: &str) -> Result<TaskKey, RouteError> {
  value.parse().map_err(|source| RouteError::TaskKey {
    value: value.to_owned(),
    source,
  })
}
