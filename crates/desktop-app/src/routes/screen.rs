use super::{CONNECTIONS, EDIT, NEW, PROJECT_PATTERN, SETTINGS, TASKS, project_screen_path};

/// A screen of the window, as the sidebar, the tabs and the app's MCP tools name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Screen {
  AllProjects,
  Settings,
  Overview,
  Tasks,
  NewTask,
  EditTask,
  Task,
  PullRequests,
  Usage,
  Pipelines,
  Agents,
  Instructions,
  Skills,
  Tools,
  Secrets,
  Connections,
  NewConnection,
  EditConnection,
  ProjectSettings,
}

impl Screen {
  pub const ALL: [Self; 19] = [
    Self::AllProjects,
    Self::Settings,
    Self::Overview,
    Self::Tasks,
    Self::NewTask,
    Self::EditTask,
    Self::Task,
    Self::PullRequests,
    Self::Usage,
    Self::Pipelines,
    Self::Agents,
    Self::Instructions,
    Self::Skills,
    Self::Tools,
    Self::Secrets,
    Self::Connections,
    Self::NewConnection,
    Self::EditConnection,
    Self::ProjectSettings,
  ];
  /// The project's screens the sidebar lists first (`Nav · Overview` … `Nav · Usage` in `q9mf0k`).
  pub const PROJECT_MAIN: [Self; 4] = [Self::Overview, Self::Tasks, Self::PullRequests, Self::Usage];
  /// The sidebar's Configure section (`Section · Configure`).
  pub const CONFIGURE: [Self; 7] = [
    Self::Pipelines,
    Self::Agents,
    Self::Instructions,
    Self::Skills,
    Self::Tools,
    Self::Secrets,
    Self::Connections,
  ];

  /// The name the app's MCP tools use for the screen.
  pub const fn name(self) -> &'static str {
    match self {
      Self::AllProjects => "all_projects",
      Self::Settings => "settings",
      Self::Overview => "overview",
      Self::Tasks => "tasks",
      Self::NewTask => "new_task",
      Self::EditTask => "edit_task",
      Self::Task => "task",
      Self::PullRequests => "pull_requests",
      Self::Usage => "usage",
      Self::Pipelines => "pipelines",
      Self::Agents => "agents",
      Self::Instructions => "instructions",
      Self::Skills => "skills",
      Self::Tools => "tools",
      Self::Secrets => "secrets",
      Self::Connections => "connections",
      Self::NewConnection => "new_connection",
      Self::EditConnection => "edit_connection",
      Self::ProjectSettings => "project_settings",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|screen| screen.name() == name)
  }

  /// i18n key of the screen's name.
  pub const fn title_key(self) -> &'static str {
    match self {
      Self::AllProjects => "shell.screens.allProjects",
      Self::Settings => "shell.screens.settings",
      Self::Overview => "shell.screens.overview",
      Self::Tasks => "shell.screens.tasks",
      Self::NewTask => "shell.screens.newTask",
      Self::EditTask => "shell.screens.editTask",
      Self::Task => "shell.screens.task",
      Self::PullRequests => "shell.screens.pullRequests",
      Self::Usage => "shell.screens.usage",
      Self::Pipelines => "shell.screens.pipelines",
      Self::Agents => "shell.screens.agents",
      Self::Instructions => "shell.screens.instructions",
      Self::Skills => "shell.screens.skills",
      Self::Tools => "shell.screens.tools",
      Self::Secrets => "shell.screens.secrets",
      Self::Connections => "shell.screens.connections",
      Self::NewConnection => "shell.screens.newConnection",
      Self::EditConnection => "shell.screens.editConnection",
      Self::ProjectSettings => "shell.screens.projectSettings",
    }
  }

  /// The segment after a project's path for a screen that names nothing but the project; `None` for its
  /// Overview, the screens of a task and the screens outside a project.
  pub(super) const fn segment(self) -> Option<&'static str> {
    match self {
      Self::AllProjects
      | Self::Settings
      | Self::Overview
      | Self::NewTask
      | Self::EditTask
      | Self::Task
      | Self::NewConnection
      | Self::EditConnection => None,
      Self::Tasks => Some(TASKS),
      Self::PullRequests => Some("pull-requests"),
      Self::Usage => Some("usage"),
      Self::Pipelines => Some("pipelines"),
      Self::Agents => Some("agents"),
      Self::Instructions => Some("instructions"),
      Self::Skills => Some("skills"),
      Self::Tools => Some("tools"),
      Self::Secrets => Some("secrets"),
      Self::Connections => Some(CONNECTIONS),
      Self::ProjectSettings => Some(SETTINGS),
    }
  }

  /// The router pattern of the screen's routes, matched without the query. A task's tabs share one
  /// (`…/tasks/:key/:tab`), so switching between them keeps the task's page; the static `new` and `edit`
  /// segments outrank its parameters.
  pub fn pattern(self) -> String {
    let tasks = format!("{PROJECT_PATTERN}/{TASKS}");
    match self {
      Self::AllProjects => "/".to_owned(),
      Self::Settings => format!("/{SETTINGS}"),
      Self::NewTask => format!("{tasks}/{NEW}"),
      Self::EditTask => format!("{tasks}/:key/{EDIT}"),
      Self::Task => format!("{tasks}/:key/:tab"),
      Self::NewConnection => format!("{PROJECT_PATTERN}/{CONNECTIONS}/{NEW}"),
      Self::EditConnection => format!("{PROJECT_PATTERN}/{CONNECTIONS}/:connection/{EDIT}"),
      screen => project_screen_path(PROJECT_PATTERN, screen),
    }
  }
}
