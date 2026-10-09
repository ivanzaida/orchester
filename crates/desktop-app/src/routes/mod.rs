//! Where the window can be: typed routes and the router paths they stand for. A path is all a tab's
//! history keeps, so everything a person can go back to is part of it.
//!
//! Paths: `/` is All projects and `/settings` the app's settings. A project's screens start at
//! `/project/<id>` (its Overview) and add the screen's segment, e.g. `/project/<id>/tasks`; a task is
//! `/project/<id>/tasks/<key>` (Conversation, the key as in `ORC-129`), with `/summary` or `/changes`
//! for its other tabs.
//!
//! A project is named by its id (`orchester_core::id::ProjectId`), not its key or name: the id never
//! changes, so a path a tab keeps still finds its project after a rename or a new key. A task is named
//! by its key (`orchester_core::project::TaskKey`), as people read it.

mod table;

use orchester_core::{id::ProjectId, project::TaskKey};

pub use self::table::routes;

const PROJECT: &str = "project";
const SETTINGS: &str = "settings";
const TASKS: &str = "tasks";
/// A project's path with its identifier as the router's parameter.
const PROJECT_PATTERN: &str = "/project/:project";

/// A screen of the window, as the sidebar, the tabs and the app's MCP tools name it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Screen {
  AllProjects,
  Settings,
  Overview,
  Tasks,
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
  ProjectSettings,
}

impl Screen {
  pub const ALL: [Self; 15] = [
    Self::AllProjects,
    Self::Settings,
    Self::Overview,
    Self::Tasks,
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
      Self::ProjectSettings => "shell.screens.projectSettings",
    }
  }

  /// The segment after a project's path; `None` for its Overview, a task and screens outside a project.
  const fn segment(self) -> Option<&'static str> {
    match self {
      Self::AllProjects | Self::Settings | Self::Overview | Self::Task => None,
      Self::Tasks => Some(TASKS),
      Self::PullRequests => Some("pull-requests"),
      Self::Usage => Some("usage"),
      Self::Pipelines => Some("pipelines"),
      Self::Agents => Some("agents"),
      Self::Instructions => Some("instructions"),
      Self::Skills => Some("skills"),
      Self::Tools => Some("tools"),
      Self::Secrets => Some("secrets"),
      Self::Connections => Some("connections"),
      Self::ProjectSettings => Some(SETTINGS),
    }
  }

  /// The router patterns of the screen's routes: one per tab for a task, one otherwise.
  pub fn patterns(self) -> Vec<String> {
    match self {
      Self::AllProjects => vec!["/".to_owned()],
      Self::Settings => vec![format!("/{SETTINGS}")],
      Self::Task => TaskTab::ALL
        .into_iter()
        .map(|tab| task_path(PROJECT_PATTERN, ":key", tab))
        .collect(),
      screen => vec![project_screen_path(PROJECT_PATTERN, screen)],
    }
  }
}

/// A tab of a task's page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TaskTab {
  #[default]
  Conversation,
  Summary,
  Changes,
}

impl TaskTab {
  pub const ALL: [Self; 3] = [Self::Conversation, Self::Summary, Self::Changes];

  /// The name the app's MCP tools use for the tab.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Conversation => "conversation",
      Self::Summary => "summary",
      Self::Changes => "changes",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }

  /// i18n key of the tab's name.
  pub const fn title_key(self) -> &'static str {
    match self {
      Self::Conversation => "task.tabs.conversation",
      Self::Summary => "task.tabs.summary",
      Self::Changes => "task.tabs.changes",
    }
  }

  /// The segment after the task's path; the Conversation is the task's own path.
  const fn segment(self) -> Option<&'static str> {
    match self {
      Self::Conversation => None,
      Self::Summary => Some("summary"),
      Self::Changes => Some("changes"),
    }
  }

  fn parse(segments: &[&str]) -> Option<Self> {
    match segments {
      [] => Some(Self::Conversation),
      [segment] => Self::ALL.into_iter().find(|tab| tab.segment() == Some(*segment)),
      _ => None,
    }
  }
}

/// A screen of one project.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectPage {
  Overview,
  Tasks,
  Task(TaskKey, TaskTab),
  PullRequests,
  Usage,
  Pipelines,
  Agents,
  Instructions,
  Skills,
  Tools,
  Secrets,
  Connections,
  Settings,
}

impl ProjectPage {
  /// The page of a project screen that names nothing but the project: every one but a task.
  pub const fn of(screen: Screen) -> Option<Self> {
    Some(match screen {
      Screen::AllProjects | Screen::Settings | Screen::Task => return None,
      Screen::Overview => Self::Overview,
      Screen::Tasks => Self::Tasks,
      Screen::PullRequests => Self::PullRequests,
      Screen::Usage => Self::Usage,
      Screen::Pipelines => Self::Pipelines,
      Screen::Agents => Self::Agents,
      Screen::Instructions => Self::Instructions,
      Screen::Skills => Self::Skills,
      Screen::Tools => Self::Tools,
      Screen::Secrets => Self::Secrets,
      Screen::Connections => Self::Connections,
      Screen::ProjectSettings => Self::Settings,
    })
  }

  pub const fn screen(&self) -> Screen {
    match self {
      Self::Overview => Screen::Overview,
      Self::Tasks => Screen::Tasks,
      Self::Task(..) => Screen::Task,
      Self::PullRequests => Screen::PullRequests,
      Self::Usage => Screen::Usage,
      Self::Pipelines => Screen::Pipelines,
      Self::Agents => Screen::Agents,
      Self::Instructions => Screen::Instructions,
      Self::Skills => Screen::Skills,
      Self::Tools => Screen::Tools,
      Self::Secrets => Screen::Secrets,
      Self::Connections => Screen::Connections,
      Self::Settings => Screen::ProjectSettings,
    }
  }

  fn path(&self, project_root: &str) -> String {
    match self {
      Self::Task(key, tab) => task_path(project_root, &key.to_string(), *tab),
      page => project_screen_path(project_root, page.screen()),
    }
  }

  fn parse(segments: &[&str]) -> Option<Self> {
    match segments {
      [] => Some(Self::Overview),
      [TASKS, key, tab @ ..] => Some(Self::Task(key.parse().ok()?, TaskTab::parse(tab)?)),
      [segment] => Screen::ALL
        .into_iter()
        .find(|screen| screen.segment() == Some(*segment))
        .and_then(Self::of),
      _ => None,
    }
  }
}

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

  /// The screen the sidebar shows as selected: a task belongs to Tasks.
  pub const fn nav_screen(&self) -> Screen {
    match self.screen() {
      Screen::Task => Screen::Tasks,
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

  /// The route at `path`, or `None` when the path names no route or holds an invalid identifier.
  pub fn parse(path: &str) -> Option<Self> {
    let rest = path.strip_prefix('/')?;
    let segments: Vec<&str> = if rest.is_empty() {
      Vec::new()
    } else {
      rest.split('/').collect()
    };
    match segments.as_slice() {
      [] => Some(Self::AllProjects),
      [SETTINGS] => Some(Self::Settings),
      [PROJECT, project, page @ ..] => Some(Self::Project(project.parse().ok()?, ProjectPage::parse(page)?)),
      _ => None,
    }
  }
}

fn project_screen_path(project_root: &str, screen: Screen) -> String {
  match screen.segment() {
    Some(segment) => format!("{project_root}/{segment}"),
    None => project_root.to_owned(),
  }
}

fn task_path(project_root: &str, key: &str, tab: TaskTab) -> String {
  let task = format!("{project_root}/{TASKS}/{key}");
  match tab.segment() {
    Some(segment) => format!("{task}/{segment}"),
    None => task,
  }
}
