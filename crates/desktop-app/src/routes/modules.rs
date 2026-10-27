//! Modules' places (ORC-112, `Modules · redesign` `oEELJ`): the list on its view (`/modules`, `?tab=closed`), New
//! module (`/modules/new`), a module's page on its tab (`/modules/<id>`, its Summary, as the page's address was
//! before it had tabs and as links to a module still read; `/modules/<id>/burndown`, and `/summary` read as the
//! Summary too), Edit module (`/modules/<id>/edit`), and New task from a module's *New task in module*
//! (`/tasks/new?module=<id>`).

use orchester_core::id::ModuleId;

use super::{
  EDIT, MODULES, NEW, ProjectPage, RouteError, TAB, TASKS,
  query::{Params, with_query},
};

/// The query parameter of the module New task starts the task in.
const MODULE: &str = "module";

/// A view of the Modules list (`Tabs` `sFKn9` in `01 · Modules`): the open modules, the one it opens on, or the
/// closed ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ModuleListTab {
  #[default]
  Open,
  Closed,
}

impl ModuleListTab {
  pub const ALL: [Self; 2] = [Self::Open, Self::Closed];

  /// The view's name: its value in a path's query, and its name in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Open => "open",
      Self::Closed => "closed",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }
}

/// A tab of a module's page (`Tabs` `kBKXR` in `02 · Module · Summary`): Summary, the one it opens on, then Burndown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ModuleTab {
  #[default]
  Summary,
  Burndown,
}

impl ModuleTab {
  pub const ALL: [Self; 2] = [Self::Summary, Self::Burndown];

  /// The tab's name: its segment in a path, and its name in the app's MCP tools.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Summary => "summary",
      Self::Burndown => "burndown",
    }
  }

  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }
}

/// The path of a Modules place under `project_root`.
pub(super) fn path(project_root: &str, page: &ProjectPage) -> Option<String> {
  let modules = format!("{project_root}/{MODULES}");
  Some(match page {
    ProjectPage::Modules(tab) => {
      let tab = (*tab != ModuleListTab::default()).then(|| tab.name());
      with_query(modules, &[(TAB, tab)])
    }
    ProjectPage::NewModule => format!("{modules}/{NEW}"),
    ProjectPage::Module(id, ModuleTab::Summary) => format!("{modules}/{id}"),
    ProjectPage::Module(id, tab) => format!("{modules}/{id}/{}", tab.name()),
    ProjectPage::EditModule(id) => format!("{modules}/{id}/{EDIT}"),
    ProjectPage::NewTaskInModule(id) => with_query(
      format!("{project_root}/{TASKS}/{NEW}"),
      &[(MODULE, Some(&id.to_string()))],
    ),
    _ => return None,
  })
}

/// The Modules place the segments after `/modules` and the query name.
pub(super) fn parse(rest: &[&str], params: &mut Params) -> Result<ProjectPage, RouteError> {
  match rest {
    [] => Ok(ProjectPage::Modules(
      params.take_or_default(TAB, ModuleListTab::from_name)?,
    )),
    [NEW] => Ok(ProjectPage::NewModule),
    [id] => Ok(ProjectPage::Module(module_id(id)?, ModuleTab::default())),
    [id, EDIT] => Ok(ProjectPage::EditModule(module_id(id)?)),
    [id, tab] => {
      let tab = ModuleTab::from_name(tab).ok_or(RouteError::NoScreen)?;
      Ok(ProjectPage::Module(module_id(id)?, tab))
    }
    _ => Err(RouteError::NoScreen),
  }
}

/// New task, in the module its query names when it names one.
pub(super) fn new_task(params: &mut Params) -> Result<ProjectPage, RouteError> {
  Ok(match params.take_parsed(MODULE)? {
    Some(module) => ProjectPage::NewTaskInModule(module),
    None => ProjectPage::NewTask,
  })
}

fn module_id(value: &str) -> Result<ModuleId, RouteError> {
  value.parse().map_err(|_| RouteError::NoScreen)
}
