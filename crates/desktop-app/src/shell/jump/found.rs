//! What *Jump to* finds for what is typed (`Results` `GMyYy` in `aDYAX`), in four groups: the tab's project's
//! tasks by key or title, its modules by name with each module's own tabs, every page and hub tab (the project's
//! and the app's), and actions. A task whose key starts with what is typed comes first. With nothing typed it
//! offers the pages and the actions. Each group shows a few results at most.

use lurq::app::ctx::Ctx;
use orchester_core::id::ProjectId;
use orchester_workspace::tasks::data::ProjectTasks;

use crate::{
  routes::{
    AccountsView, AgentTab, ConnectionsView, ModuleTab, OrchesterView, OverviewTab, ProjectPage, ProjectSettingsTab,
    Route, Screen, SecretsView, TaskView,
  },
  screens::hub::HubTab,
  shell::sidebar::place::Place,
  ui::icon::{Icon, screen_icon},
};

/// How many results each group shows at most.
const TASKS_SHOWN: usize = 5;
const MODULES_SHOWN: usize = 4;
const PAGES_SHOWN: usize = 6;
const ACTIONS_SHOWN: usize = 4;

/// A group of results, in the palette's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
  Tasks,
  Modules,
  Pages,
  Actions,
}

impl Group {
  pub const ALL: [Self; 4] = [Self::Tasks, Self::Modules, Self::Pages, Self::Actions];

  pub const fn name(self) -> &'static str {
    match self {
      Self::Tasks => "tasks",
      Self::Modules => "modules",
      Self::Pages => "pages",
      Self::Actions => "actions",
    }
  }

  pub const fn title_key(self) -> &'static str {
    match self {
      Self::Tasks => "shell.jump.groups.tasks",
      Self::Modules => "shell.jump.groups.modules",
      Self::Pages => "shell.jump.groups.pages",
      Self::Actions => "shell.jump.groups.actions",
    }
  }
}

/// Where a result goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
  Route(Route),
  /// A new tab at All projects.
  NewTab,
}

/// A result: its group, its element id's part, its icon, its words and where it goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
  pub group: Group,
  pub id: String,
  pub icon: Icon,
  pub label: String,
  pub target: Target,
}

/// A page, a module's tab or an action before it is matched.
struct Entry {
  id: String,
  icon: Icon,
  label: String,
  target: Target,
}

impl Entry {
  fn new(id: impl Into<String>, icon: Icon, label: String, target: Target) -> Self {
    Self {
      id: id.into(),
      icon,
      label,
      target,
    }
  }
}

/// The results for `query` in the tab's `project`, whose tasks and modules are `tasks` once read.
pub fn find(ctx: &Ctx, project: Option<ProjectId>, tasks: Option<&ProjectTasks>, query: &str) -> Vec<Found> {
  let query = query.trim().to_lowercase();
  let mut found = Vec::new();
  if let (Some(project), Some(tasks), false) = (project, tasks, query.is_empty()) {
    found.extend(task_results(ctx, project, tasks, &query));
    found.extend(matching(
      Group::Modules,
      module_entries(ctx, project, tasks),
      &query,
      MODULES_SHOWN,
    ));
  }
  let mut pages = Vec::new();
  if let Some(project) = project {
    pages.extend(places(ctx, project));
    pages.extend(hub_tabs(ctx, project));
    pages.extend(settings_tabs(ctx, project));
  }
  pages.extend(app_pages(ctx));
  found.extend(matching(Group::Pages, pages, &query, PAGES_SHOWN));
  found.extend(matching(Group::Actions, actions(ctx, project), &query, ACTIONS_SHOWN));
  found
}

fn word(ctx: &Ctx, key: &str) -> String {
  ctx.t(key).to_string()
}

/// `page` › the words of `tab`.
fn within(ctx: &Ctx, page: &str, tab: &str) -> String {
  let tab = ctx.t(tab);
  ctx
    .t_args("shell.jump.within", [("page", page), ("tab", &*tab)])
    .to_string()
}

/// The entries whose words hold `query`, ignoring case, `shown` at most.
fn matching(group: Group, entries: Vec<Entry>, query: &str, shown: usize) -> impl Iterator<Item = Found> + use<> {
  let query = query.to_owned();
  entries
    .into_iter()
    .filter(move |entry| entry.label.to_lowercase().contains(&query))
    .take(shown)
    .map(move |entry| Found {
      group,
      id: entry.id,
      icon: entry.icon,
      label: entry.label,
      target: entry.target,
    })
}

/// The tasks whose key or title holds `query`: those whose key starts with it first.
fn task_results(ctx: &Ctx, project: ProjectId, tasks: &ProjectTasks, query: &str) -> Vec<Found> {
  let mut hits: Vec<(bool, Found)> = tasks
    .tasks
    .iter()
    .filter_map(|task| {
      let key = task.key().to_string();
      let title = task.details().title.as_str();
      let lower = key.to_lowercase();
      let by_key = lower.starts_with(query);
      if !by_key && !lower.contains(query) && !title.to_lowercase().contains(query) {
        return None;
      }
      let page = ProjectPage::task(task.key().clone(), TaskView::default(), false);
      let label = ctx.t_args("shell.jump.task", [("key", key.as_str()), ("title", title)]);
      let found = Found {
        group: Group::Tasks,
        id: format!("task-{key}"),
        icon: screen_icon(Screen::Task),
        label: label.to_string(),
        target: Target::Route(Route::Project(project, page)),
      };
      Some((by_key, found))
    })
    .collect();
  // A stable sort: the tasks keep the list's order within each kind of match.
  hits.sort_by_key(|(by_key, _)| !*by_key);
  hits.into_iter().take(TASKS_SHOWN).map(|(_, found)| found).collect()
}

/// Each module's page, and its other tabs as *Module › Burndown*.
fn module_entries(ctx: &Ctx, project: ProjectId, tasks: &ProjectTasks) -> Vec<Entry> {
  let mut entries = Vec::new();
  for module in tasks.modules.iter() {
    let name = module.name().as_str();
    for tab in ModuleTab::ALL {
      let label = match tab {
        ModuleTab::Summary => name.to_owned(),
        ModuleTab::Burndown => within(ctx, name, "modules.page.tabs.burndown"),
      };
      let page = ProjectPage::Module(module.id(), tab);
      entries.push(Entry::new(
        format!("module-{}-{}", module.id(), tab.name()),
        screen_icon(Screen::Modules),
        label,
        Target::Route(Route::Project(project, page)),
      ));
    }
  }
  entries
}

/// The sidebar's places other than the hubs, and Usage.
fn places(ctx: &Ctx, project: ProjectId) -> Vec<Entry> {
  [
    Place::Overview,
    Place::Tasks,
    Place::Modules,
    Place::Orchestrator,
    Place::Files,
    Place::Pipelines,
  ]
  .into_iter()
  .map(|place| {
    Entry::new(
      place.name(),
      place.icon(),
      word(ctx, place.title_key()),
      Target::Route(place.route(project)),
    )
  })
  .chain([Entry::new(
    "usage",
    screen_icon(Screen::Usage),
    word(ctx, "shell.screens.usage"),
    Target::Route(Route::Project(project, ProjectPage::Usage(Box::default()))),
  )])
  .collect()
}

/// The Review hub's and the Agents hub's tabs, and the Overview's.
fn hub_tabs(ctx: &Ctx, project: ProjectId) -> Vec<Entry> {
  let at = |page: ProjectPage| Target::Route(Route::Project(project, page));
  let review = word(ctx, "shell.screens.review");
  let mut entries = vec![
    Entry::new(
      "merge-queue",
      Place::Review.icon(),
      within(ctx, &review, "shell.screens.mergeQueue"),
      at(ProjectPage::MergeQueue),
    ),
    Entry::new(
      "pull-requests",
      Place::Review.icon(),
      within(ctx, &review, "shell.screens.pullRequests"),
      at(ProjectPage::PullRequests(Default::default())),
    ),
  ];
  let agents = word(ctx, "shell.screens.agents");
  entries.extend(HubTab::ALL.into_iter().map(|tab| {
    Entry::new(
      format!("agents-{}", tab.name()),
      Place::Agents.icon(),
      within(ctx, &agents, tab.label_key()),
      Target::Route(tab.route(project)),
    )
  }));
  let overview = word(ctx, "shell.screens.overview");
  entries.extend(
    [
      (OverviewTab::Running, "overview.tabs.running"),
      (OverviewTab::NeedsYou, "overview.tabs.needsYou"),
      (OverviewTab::Queued, "overview.tabs.queued"),
      (OverviewTab::Failed, "overview.tabs.failed"),
    ]
    .map(|(tab, key)| {
      Entry::new(
        format!("overview-{}", tab.name()),
        Place::Overview.icon(),
        within(ctx, &overview, key),
        at(ProjectPage::Overview(tab)),
      )
    }),
  );
  entries
}

/// Project settings' tabs, Connections and Secrets among them.
fn settings_tabs(ctx: &Ctx, project: ProjectId) -> Vec<Entry> {
  let settings = word(ctx, "shell.screens.projectSettings");
  [
    (
      "general",
      "project_settings.general",
      ProjectPage::Settings(ProjectSettingsTab::General),
    ),
    (
      "runs",
      "project_settings.runs",
      ProjectPage::Settings(ProjectSettingsTab::Runs),
    ),
    (
      "context",
      "project_settings.context",
      ProjectPage::Settings(ProjectSettingsTab::Context),
    ),
    (
      "connections",
      "project_settings.connections",
      ProjectPage::Connections(ConnectionsView::default()),
    ),
    (
      "secrets",
      "project_settings.secrets",
      ProjectPage::Secrets(SecretsView::default()),
    ),
    (
      "sharing",
      "project_settings.sharing",
      ProjectPage::Settings(ProjectSettingsTab::Sharing),
    ),
  ]
  .into_iter()
  .map(|(id, key, page)| {
    Entry::new(
      format!("project-settings-{id}"),
      screen_icon(page.screen()),
      within(ctx, &settings, key),
      Target::Route(Route::Project(project, page)),
    )
  })
  .collect()
}

/// All projects and the app's Settings with its tabs.
fn app_pages(ctx: &Ctx) -> Vec<Entry> {
  let settings = word(ctx, "shell.screens.settings");
  let tabs = [
    ("automation", "settings.tabs.automation", Route::Settings),
    ("general", "settings.tabs.general", Route::General),
    ("notifications", "settings.tabs.notifications", Route::Notifications),
    (
      "accounts",
      "settings.tabs.accounts",
      Route::Accounts(AccountsView::default()),
    ),
    (
      "orchester",
      "settings.tabs.orchester",
      Route::Orchester(OrchesterView::default()),
    ),
  ]
  .map(|(id, key, route)| {
    Entry::new(
      format!("settings-{id}"),
      Icon::Settings,
      within(ctx, &settings, key),
      Target::Route(route),
    )
  });
  let all = Entry::new(
    "all-projects",
    Icon::Folders,
    word(ctx, "shell.screens.allProjects"),
    Target::Route(Route::AllProjects),
  );
  std::iter::once(all).chain(tabs).collect()
}

/// What can be started from anywhere: the project's new task, module, pipeline and agent profile, then a new tab
/// and Add project.
fn actions(ctx: &Ctx, project: Option<ProjectId>) -> Vec<Entry> {
  let mut entries = Vec::new();
  if let Some(project) = project {
    let at = |page: ProjectPage| Target::Route(Route::Project(project, page));
    entries.extend([
      (
        "new-task",
        Icon::Plus,
        "shell.jump.actions.newTask",
        at(ProjectPage::NewTask),
      ),
      (
        "new-module",
        Icon::Plus,
        "shell.jump.actions.newModule",
        at(ProjectPage::NewModule),
      ),
      (
        "new-pipeline",
        Icon::Plus,
        "shell.jump.actions.newPipeline",
        at(ProjectPage::NewPipeline),
      ),
      (
        "new-agent",
        Icon::Plus,
        "shell.jump.actions.newAgent",
        at(ProjectPage::NewAgent(AgentTab::default())),
      ),
    ]);
  }
  entries.extend([
    ("new-tab", Icon::AppWindow, "shell.jump.actions.newTab", Target::NewTab),
    (
      "add-project",
      Icon::FolderPlus,
      "shell.jump.actions.addProject",
      Target::Route(Route::AddProject(None)),
    ),
  ]);
  entries
    .into_iter()
    .map(|(id, icon, key, target)| Entry::new(id, icon, word(ctx, key), target))
    .collect()
}
