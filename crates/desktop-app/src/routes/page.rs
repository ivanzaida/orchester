use orchester_core::project::TaskKey;

use super::{
  EDIT, NEW, OverviewTab, RouteError, Screen, TAB, TASKS, TaskList, TaskView, project_screen_path,
  query::{Params, with_query},
};

/// A screen of one project, with what its path names besides the project.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectPage {
  Overview(OverviewTab),
  Tasks(TaskList),
  NewTask,
  EditTask(TaskKey),
  Task(TaskKey, TaskView),
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
  /// The page of a project screen that names nothing but the project, as it opens: every one but the
  /// screens of a task.
  pub fn of(screen: Screen) -> Option<Self> {
    Some(match screen {
      Screen::AllProjects | Screen::Settings | Screen::EditTask | Screen::Task => return None,
      Screen::Overview => Self::Overview(OverviewTab::default()),
      Screen::Tasks => Self::Tasks(TaskList::default()),
      Screen::NewTask => Self::NewTask,
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
      Self::Overview(_) => Screen::Overview,
      Self::Tasks(_) => Screen::Tasks,
      Self::NewTask => Screen::NewTask,
      Self::EditTask(_) => Screen::EditTask,
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

  pub(super) fn path(&self, project_root: &str) -> String {
    let tasks = format!("{project_root}/{TASKS}");
    match self {
      Self::Overview(tab) => {
        let tab = (*tab != OverviewTab::default()).then(|| tab.name());
        with_query(project_root.to_owned(), &[(TAB, tab)])
      }
      Self::Tasks(list) => list.path(tasks),
      Self::NewTask => format!("{tasks}/{NEW}"),
      Self::EditTask(key) => format!("{tasks}/{key}/{EDIT}"),
      Self::Task(key, view) => view.path(&format!("{tasks}/{key}")),
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
