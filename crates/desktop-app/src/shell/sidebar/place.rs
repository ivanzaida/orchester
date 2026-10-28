//! The sidebar's entries (`Sidebar v2` `Zn8dR` in `Sidebar · redesign` `WWXmX`): six places for daily work, and
//! the two hubs of the Configure section. Each opens one route and stays selected on every route that belongs to
//! it (`Where each entry went` `j3WMs9`): a task and its forms are Tasks', a module's page and its forms Modules',
//! the Merge queue and the Pull requests with a pull request's page the Review hub's, and Profiles, Instructions,
//! Skills, Tools and Memory the Agents hub's. Usage, Project settings with its Connections and Secrets tabs, and
//! the app's Settings have no entry: the usage card and the switcher's menu open them.

use orchester_core::id::ProjectId;

use crate::{
  routes::{FilesView, ModuleListTab, OrchestratorChat, OverviewTab, ProjectPage, Route, Screen, TaskList},
  ui::icon::{Icon, screen_icon},
};

/// An entry of the sidebar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
  Overview,
  Tasks,
  Modules,
  Orchestrator,
  Review,
  Files,
  Pipelines,
  Agents,
}

impl Place {
  /// The places for daily work, in the sidebar's order.
  pub const DAILY: [Self; 6] = [
    Self::Overview,
    Self::Tasks,
    Self::Modules,
    Self::Orchestrator,
    Self::Review,
    Self::Files,
  ];
  /// The Configure section's hubs.
  pub const CONFIGURE: [Self; 2] = [Self::Pipelines, Self::Agents];

  /// The entry selected on `route`, if any.
  pub const fn of(route: &Route) -> Option<Self> {
    if route.project().is_none() {
      return None;
    }
    Some(match route.screen() {
      Screen::Overview => Self::Overview,
      Screen::Tasks | Screen::Task | Screen::NewTask | Screen::EditTask => Self::Tasks,
      Screen::Modules => Self::Modules,
      Screen::Orchestrator | Screen::OrchestratorSettings => Self::Orchestrator,
      Screen::MergeQueue | Screen::PullRequests => Self::Review,
      Screen::Files => Self::Files,
      Screen::Pipelines => Self::Pipelines,
      Screen::Agents | Screen::Instructions | Screen::Memory | Screen::Skills | Screen::Tools => Self::Agents,
      _ => return None,
    })
  }

  /// Where the entry goes in `project`: the Review hub on its Merge queue, the Agents hub on its Profiles.
  pub fn route(self, project: ProjectId) -> Route {
    let page = match self {
      Self::Overview => ProjectPage::Overview(OverviewTab::default()),
      Self::Tasks => ProjectPage::Tasks(TaskList::default()),
      Self::Modules => ProjectPage::Modules(ModuleListTab::default()),
      Self::Orchestrator => ProjectPage::Orchestrator(OrchestratorChat::default()),
      Self::Review => ProjectPage::MergeQueue,
      Self::Files => ProjectPage::Files(FilesView::default()),
      Self::Pipelines => ProjectPage::Pipelines,
      Self::Agents => ProjectPage::Agents,
    };
    Route::Project(project, page)
  }

  /// The entry's name in its element id, `nav-<name>`.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Overview => "overview",
      Self::Tasks => "tasks",
      Self::Modules => "modules",
      Self::Orchestrator => "orchestrator",
      Self::Review => "review",
      Self::Files => "files",
      Self::Pipelines => "pipelines",
      Self::Agents => "agents",
    }
  }

  pub const fn title_key(self) -> &'static str {
    match self {
      Self::Review => "shell.screens.review",
      place => place.screen().title_key(),
    }
  }

  pub const fn icon(self) -> Icon {
    match self {
      Self::Review => screen_icon(Screen::PullRequests),
      place => screen_icon(place.screen()),
    }
  }

  /// The screen the entry opens; the Review hub's is its first tab's.
  const fn screen(self) -> Screen {
    match self {
      Self::Overview => Screen::Overview,
      Self::Tasks => Screen::Tasks,
      Self::Modules => Screen::Modules,
      Self::Orchestrator => Screen::Orchestrator,
      Self::Review => Screen::MergeQueue,
      Self::Files => Screen::Files,
      Self::Pipelines => Screen::Pipelines,
      Self::Agents => Screen::Agents,
    }
  }
}
