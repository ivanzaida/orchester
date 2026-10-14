use orchester_core::{
  id::{ConnectionId, ResourceId},
  project::TaskKey,
};

use super::{
  CONNECTIONS, ConnectionsView, EDIT, NEW, NewConnection, OverviewTab, RouteError, Screen, TAB, TASKS, TaskList,
  TaskView, ToolTab, project_screen_path,
  query::{Params, with_query},
};

/// A screen of one project, with what its path names besides the project.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectPage {
  Overview(OverviewTab),
  Orchestrator,
  Tasks(TaskList),
  NewTask,
  EditTask(TaskKey),
  Task(TaskKey, TaskView),
  MergeQueue,
  PullRequests,
  Usage,
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
      Screen::Orchestrator => Self::Orchestrator,
      Screen::Tasks => Self::Tasks(TaskList::default()),
      Screen::NewTask => Self::NewTask,
      Screen::MergeQueue => Self::MergeQueue,
      Screen::PullRequests => Self::PullRequests,
      Screen::Usage => Self::Usage,
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
    })
  }

  pub const fn screen(&self) -> Screen {
    match self {
      Self::Overview(_) => Screen::Overview,
      Self::Orchestrator => Screen::Orchestrator,
      Self::Tasks(_) => Screen::Tasks,
      Self::NewTask => Screen::NewTask,
      Self::EditTask(_) => Screen::EditTask,
      Self::Task(..) => Screen::Task,
      Self::MergeQueue => Screen::MergeQueue,
      Self::PullRequests => Screen::PullRequests,
      Self::Usage => Screen::Usage,
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
      Self::Settings => Screen::ProjectSettings,
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
      Self::Connections(view) => view.path(connections),
      Self::NewConnection(kind) => kind.path(format!("{connections}/{NEW}")),
      Self::EditConnection(id) => format!("{connections}/{id}/{EDIT}"),
      page => project_screen_path(project_root, page.screen()),
    }
  }

  /// The page the segments after a project's path and the query name.
  pub(super) fn parse(segments: &[&str], params: &mut Params) -> Result<Self, RouteError> {
    match segments {
      [] => Ok(Self::Overview(params.take_or_default(TAB, OverviewTab::from_name)?)),
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
      [CONNECTIONS] => ConnectionsView::take(params).map(Self::Connections),
      [CONNECTIONS, NEW] => NewConnection::take(params).map(Self::NewConnection),
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
