use super::{
  ADD_PROJECT, CONNECTIONS, EDIT, IMPORT, MODULES, NEW, ORCHESTRATOR, PROJECT_PATTERN, PULL_REQUESTS, SETTINGS, TASKS,
  files::FILES, project_screen_path,
};

/// A screen of the window, as the sidebar, the tabs and the app's MCP tools name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Screen {
  AllProjects,
  AddProject,
  Settings,
  Overview,
  Orchestrator,
  OrchestratorSettings,
  Tasks,
  NewTask,
  EditTask,
  Task,
  /// The Modules list and a module's page.
  Modules,
  MergeQueue,
  PullRequests,
  Usage,
  Pipelines,
  Agents,
  Instructions,
  Memory,
  Skills,
  Tools,
  Secrets,
  Connections,
  NewConnection,
  EditConnection,
  ProjectSettings,
  /// The code viewer.
  Files,
  /// Import resources, which no sidebar item names (`Baggg` in `G69AHh`).
  ImportResources,
}

impl Screen {
  pub const ALL: [Self; 27] = [
    Self::AllProjects,
    Self::AddProject,
    Self::Settings,
    Self::Overview,
    Self::Orchestrator,
    Self::OrchestratorSettings,
    Self::Tasks,
    Self::NewTask,
    Self::EditTask,
    Self::Task,
    Self::Modules,
    Self::MergeQueue,
    Self::PullRequests,
    Self::Usage,
    Self::Pipelines,
    Self::Agents,
    Self::Instructions,
    Self::Memory,
    Self::Skills,
    Self::Tools,
    Self::Secrets,
    Self::Connections,
    Self::NewConnection,
    Self::EditConnection,
    Self::ProjectSettings,
    Self::Files,
    Self::ImportResources,
  ];
  /// The project's screens the sidebar lists first (`Nav · Overview` … `Nav · Usage` in `q9mf0k`), Modules under
  /// Tasks (ORC-48, no frame yet), and Files under Usage (`Nav · Files` `E0b2rn` in `o1Uea4`).
  pub const PROJECT_MAIN: [Self; 8] = [
    Self::Overview,
    Self::Orchestrator,
    Self::Tasks,
    Self::Modules,
    Self::MergeQueue,
    Self::PullRequests,
    Self::Usage,
    Self::Files,
  ];
  /// The sidebar's Configure section (`Section · Configure`). Secrets and Connections are tabs of Project settings.
  pub const CONFIGURE: [Self; 6] = [
    Self::Pipelines,
    Self::Agents,
    Self::Instructions,
    Self::Memory,
    Self::Skills,
    Self::Tools,
  ];

  /// The name the app's MCP tools use for the screen.
  pub const fn name(self) -> &'static str {
    match self {
      Self::AllProjects => "all_projects",
      Self::AddProject => "add_project",
      Self::Settings => "settings",
      Self::Overview => "overview",
      Self::Orchestrator => "orchestrator",
      Self::OrchestratorSettings => "orchestrator_settings",
      Self::Tasks => "tasks",
      Self::NewTask => "new_task",
      Self::EditTask => "edit_task",
      Self::Task => "task",
      Self::Modules => "modules",
      Self::MergeQueue => "merge_queue",
      Self::PullRequests => "pull_requests",
      Self::Usage => "usage",
      Self::Pipelines => "pipelines",
      Self::Agents => "agents",
      Self::Instructions => "instructions",
      Self::Memory => "memory",
      Self::Skills => "skills",
      Self::Tools => "tools",
      Self::Secrets => "secrets",
      Self::Connections => "connections",
      Self::NewConnection => "new_connection",
      Self::EditConnection => "edit_connection",
      Self::ProjectSettings => "project_settings",
      Self::Files => "files",
      Self::ImportResources => "import_resources",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|screen| screen.name() == name)
  }

  /// i18n key of the screen's name.
  pub const fn title_key(self) -> &'static str {
    match self {
      Self::AllProjects => "shell.screens.allProjects",
      Self::AddProject => "shell.screens.addProject",
      Self::Settings => "shell.screens.settings",
      Self::Overview => "shell.screens.overview",
      Self::Orchestrator => "shell.screens.orchestrator",
      Self::OrchestratorSettings => "shell.screens.orchestratorSettings",
      Self::Tasks => "shell.screens.tasks",
      Self::NewTask => "shell.screens.newTask",
      Self::EditTask => "shell.screens.editTask",
      Self::Task => "shell.screens.task",
      Self::Modules => "shell.screens.modules",
      Self::MergeQueue => "shell.screens.mergeQueue",
      Self::PullRequests => "shell.screens.pullRequests",
      Self::Usage => "shell.screens.usage",
      Self::Pipelines => "shell.screens.pipelines",
      Self::Agents => "shell.screens.agents",
      Self::Instructions => "shell.screens.instructions",
      Self::Memory => "shell.screens.memory",
      Self::Skills => "shell.screens.skills",
      Self::Tools => "shell.screens.tools",
      Self::Secrets => "shell.screens.secrets",
      Self::Connections => "shell.screens.connections",
      Self::NewConnection => "shell.screens.newConnection",
      Self::EditConnection => "shell.screens.editConnection",
      Self::ProjectSettings => "shell.screens.projectSettings",
      Self::Files => "shell.screens.files",
      Self::ImportResources => "shell.screens.importResources",
    }
  }

  /// The segment after a project's path for a screen that names nothing but the project; `None` for its
  /// Overview, the screens of a task and the screens outside a project.
  pub(super) const fn segment(self) -> Option<&'static str> {
    match self {
      Self::AllProjects
      | Self::AddProject
      | Self::Settings
      | Self::Overview
      | Self::NewTask
      | Self::EditTask
      | Self::Task
      | Self::NewConnection
      | Self::EditConnection
      | Self::OrchestratorSettings => None,
      Self::Orchestrator => Some(ORCHESTRATOR),
      Self::Tasks => Some(TASKS),
      Self::Modules => Some(MODULES),
      Self::MergeQueue => Some("merge-queue"),
      Self::PullRequests => Some(PULL_REQUESTS),
      Self::Usage => Some("usage"),
      Self::Pipelines => Some("pipelines"),
      Self::Agents => Some("agents"),
      Self::Instructions => Some("instructions"),
      Self::Memory => Some("memory"),
      Self::Skills => Some("skills"),
      Self::Tools => Some("tools"),
      Self::Secrets => Some("secrets"),
      Self::Connections => Some(CONNECTIONS),
      Self::ProjectSettings => Some(SETTINGS),
      Self::Files => Some(FILES),
      Self::ImportResources => Some(IMPORT),
    }
  }

  /// The router pattern of the screen's routes, matched without the query. A task's tabs share one
  /// (`…/tasks/:key/:tab`), so switching between them keeps the task's page; the static `new` and `edit`
  /// segments outrank its parameters.
  pub fn pattern(self) -> String {
    let tasks = format!("{PROJECT_PATTERN}/{TASKS}");
    match self {
      Self::AllProjects => "/".to_owned(),
      Self::AddProject => format!("/{ADD_PROJECT}"),
      Self::Settings => format!("/{SETTINGS}"),
      Self::NewTask => format!("{tasks}/{NEW}"),
      Self::EditTask => format!("{tasks}/:key/{EDIT}"),
      Self::Task => format!("{tasks}/:key/:tab"),
      Self::NewConnection => format!("{PROJECT_PATTERN}/{CONNECTIONS}/{NEW}"),
      Self::EditConnection => format!("{PROJECT_PATTERN}/{CONNECTIONS}/:connection/{EDIT}"),
      Self::OrchestratorSettings => format!("{PROJECT_PATTERN}/{ORCHESTRATOR}/{SETTINGS}"),
      screen => project_screen_path(PROJECT_PATTERN, screen),
    }
  }
}
