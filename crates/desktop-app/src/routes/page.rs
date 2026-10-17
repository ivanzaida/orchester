use std::num::NonZeroU64;

use orchester_core::{
  id::{ConnectionId, ResourceId},
  project::TaskKey,
  resource::VersionNumber,
};
use orchester_workspace::{links::USAGE, pull_requests::list::PullRequestTab, usage::UsageQuery};

use super::{
  AgentTab, CONNECTIONS, ConnectionsView, EDIT, FilesView, InstructionsView, NEW, NewConnection, ORCHESTRATOR,
  OrchestratorChat, OverviewTab, PULL_REQUESTS, PipelinePanel, ProjectSettingsTab, RouteError, SETTINGS, Screen,
  SecretsView, SkillsFilter, TAB, TASKS, TaskList, TaskView, ToolTab,
  files::FILES,
  project_screen_path,
  query::{Params, with_query},
};

/// The query parameter of the file a pull request's page shows.
const FILE: &str = "file";
/// What Project settings' Sharing tab asks once open.
const ASK: &str = "ask";
const MAKE_LOCAL: &str = "make-local";

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
  /// A pull request of the project's repository, by its number, with the path of the file whose diff it
  /// shows (`None`: the first).
  PullRequest(NonZeroU64, Option<String>),
  /// The Usage page with its period, grouping and filters (boxed: a query holds many filters).
  Usage(Box<UsageQuery>),
  Pipelines,
  NewPipeline,
  NewEmptyPipeline,
  /// A pipeline's editor, with the panel it shows beside the graph.
  Pipeline(ResourceId, PipelinePanel),
  /// A pipeline's editor showing a published version, read-only, picked in its history, with the panel
  /// beside the graph; never the publish review.
  PipelineVersion(ResourceId, VersionNumber, PipelinePanel),
  Agents,
  NewAgent(AgentTab),
  Agent(ResourceId, AgentTab),
  Instructions(InstructionsView),
  Memory,
  /// The Skills library with its filter.
  Skills(SkillsFilter),
  NewSkill,
  Skill(ResourceId),
  ImportSkills,
  Tools,
  NewTool,
  Tool(ResourceId, ToolTab),
  Secrets(SecretsView),
  Connections(ConnectionsView),
  NewConnection(NewConnection),
  /// Edit MCP server, the server's id.
  EditConnection(ConnectionId),
  Settings(ProjectSettingsTab),
  /// Project settings on its Sharing tab with *Make local*'s question open (the task list's *no longer a
  /// member* notice).
  MakeLocal,
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
      Screen::Instructions => Self::Instructions(InstructionsView::default()),
      Screen::Memory => Self::Memory,
      Screen::Skills => Self::Skills(SkillsFilter::default()),
      Screen::Tools => Self::Tools,
      Screen::Secrets => Self::Secrets(SecretsView::default()),
      Screen::Connections => Self::Connections(ConnectionsView::default()),
      Screen::NewConnection => Self::NewConnection(NewConnection::default()),
      Screen::ProjectSettings => Self::Settings(ProjectSettingsTab::default()),
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
      Self::PullRequests(_) | Self::PullRequest(..) => Screen::PullRequests,
      Self::Usage(_) => Screen::Usage,
      Self::Pipelines | Self::NewPipeline | Self::NewEmptyPipeline | Self::Pipeline(..) | Self::PipelineVersion(..) => {
        Screen::Pipelines
      }
      Self::Agents | Self::NewAgent(_) | Self::Agent(..) => Screen::Agents,
      Self::Instructions(_) => Screen::Instructions,
      Self::Memory => Screen::Memory,
      Self::Skills(_) | Self::NewSkill | Self::Skill(_) | Self::ImportSkills => Screen::Skills,
      Self::Tools | Self::NewTool | Self::Tool(..) => Screen::Tools,
      Self::Secrets(_) => Screen::Secrets,
      Self::Connections(_) => Screen::Connections,
      Self::NewConnection(_) => Screen::NewConnection,
      Self::EditConnection(_) => Screen::EditConnection,
      Self::Settings(_) | Self::MakeLocal => Screen::ProjectSettings,
      Self::Files(_) => Screen::Files,
    }
  }

  pub(super) fn path(&self, project_root: &str) -> String {
    let tasks = format!("{project_root}/{TASKS}");
    let connections = format!("{project_root}/{CONNECTIONS}");
    let segment = |screen: Screen| project_screen_path(project_root, screen);
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
      Self::Skills(filter) => with_query(segment(Screen::Skills), &[filter.param()]),
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
      Self::Pipeline(id, panel) => panel.path(format!("{project_root}/pipelines/{id}"), None),
      Self::PipelineVersion(id, version, panel) => panel.path(format!("{project_root}/pipelines/{id}"), Some(*version)),
      Self::NewAgent(tab) => with_query(format!("{project_root}/agents/{NEW}"), &[tab.param()]),
      Self::Agent(id, tab) => with_query(format!("{project_root}/agents/{id}"), &[tab.param()]),
      Self::Instructions(view) => view.path(segment(Screen::Instructions)),
      Self::Secrets(view) => view.path(segment(Screen::Secrets)),
      Self::Settings(tab) => with_query(segment(Screen::ProjectSettings), &[tab.param()]),
      Self::PullRequests(tab) => {
        let tab = (*tab != PullRequestTab::default()).then(|| tab.name());
        with_query(format!("{project_root}/{PULL_REQUESTS}"), &[(TAB, tab)])
      }
      Self::PullRequest(number, file) => with_query(
        format!("{project_root}/{PULL_REQUESTS}/{number}"),
        &[(FILE, file.as_deref())],
      ),
      Self::Usage(query) => super::usage::path(project_root, query),
      Self::Connections(view) => view.path(connections),
      Self::NewConnection(kind) => kind.path(format!("{connections}/{NEW}")),
      Self::EditConnection(id) => format!("{connections}/{id}/{EDIT}"),
      Self::Files(view) => view.path(format!("{project_root}/{FILES}")),
      Self::MakeLocal => with_query(
        segment(Screen::ProjectSettings),
        &[ProjectSettingsTab::Sharing.param(), (ASK, Some(MAKE_LOCAL))],
      ),
      page => segment(page.screen()),
    }
  }

  /// The page the segments after a project's path and the query name.
  pub(super) fn parse(segments: &[&str], params: &mut Params) -> Result<Self, RouteError> {
    match segments {
      [] => Ok(Self::Overview(params.take_or_default(TAB, OverviewTab::from_name)?)),
      [ORCHESTRATOR] => OrchestratorChat::take(params).map(Self::Orchestrator),
      [ORCHESTRATOR, SETTINGS] => Ok(Self::OrchestratorSettings),
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
        None => SkillsFilter::take(params).map(Self::Skills),
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
      ["pipelines", id] => {
        let id = id.parse().map_err(|_| RouteError::NoScreen)?;
        let version = PipelinePanel::take_version(params)?;
        match (version, PipelinePanel::take(params)?) {
          (None, panel) => Ok(Self::Pipeline(id, panel)),
          // A published version is never published again.
          (Some(_), PipelinePanel::Publish) => Err(RouteError::InvalidParam {
            param: "panel".to_owned(),
            value: "publish".to_owned(),
          }),
          (Some(version), panel) => Ok(Self::PipelineVersion(id, version, panel)),
        }
      }
      ["agents", NEW] => AgentTab::take(params).map(Self::NewAgent),
      ["agents", id] => {
        let id = id.parse().map_err(|_| RouteError::NoScreen)?;
        AgentTab::take(params).map(|tab| Self::Agent(id, tab))
      }
      ["instructions"] => InstructionsView::take(params).map(Self::Instructions),
      ["secrets"] => SecretsView::take(params).map(Self::Secrets),
      [SETTINGS] => settings(params),
      [PULL_REQUESTS] => Ok(Self::PullRequests(
        params.take_or_default(TAB, PullRequestTab::from_name)?,
      )),
      [USAGE] => super::usage::take(params).map(|query| Self::Usage(Box::new(query))),
      [PULL_REQUESTS, number] => {
        let number = number.parse::<NonZeroU64>().map_err(|_| RouteError::NoScreen)?;
        Ok(Self::PullRequest(number, params.take(FILE)?))
      }
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

/// Project settings on the tab the query names, or *Make local*'s question on Sharing (`ask=make-local`).
fn settings(params: &mut Params) -> Result<ProjectPage, RouteError> {
  let tab = ProjectSettingsTab::take(params)?;
  match (tab, params.take(ASK)?) {
    (_, None) => Ok(ProjectPage::Settings(tab)),
    (ProjectSettingsTab::Sharing, Some(ask)) if ask == MAKE_LOCAL => Ok(ProjectPage::MakeLocal),
    (_, Some(value)) => Err(RouteError::InvalidParam {
      param: ASK.to_owned(),
      value,
    }),
  }
}
