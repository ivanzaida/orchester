//! What the window opens with (design doc, "Tabs" › "Restore"): the tabs the last session saved —
//! their order, locations and the active one, without their history — or, the first time, the
//! start screen in one tab. A saved tab whose project can no longer be opened (folder missing,
//! newer data) opens All projects, which says why, never an error page. Blocks on the
//! application and project databases; the root reads it off the UI thread.

use orchester_core::id::ProjectId;
use orchester_store::{RegisteredProject, StoreError};

use super::model::{SavedTabs, TABS_KEY, TabSet};
use crate::{
  navigation::{Places, Start, route::Route},
  workspace::Workspace,
};

/// The window's first tabs, and the project the active one shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Opening {
  pub set: TabSet,
  pub shown_project: Option<ProjectId>,
}

impl lurq::app::component::DevtoolsInspectable for Opening {}

/// Reads the saved tabs, opening the projects they show into `places`, or else the start screen.
pub fn opening(workspace: &Workspace, places: &Places) -> Result<Opening, StoreError> {
  let home = Route::AllProjects.path();
  let saved = saved_tabs(workspace)?;
  let restored = match saved {
    Some(saved) => restore(workspace, places, &saved)?,
    None => None,
  };
  let set = match restored {
    Some(set) => set,
    None => {
      let start = Start::from(workspace.landing()?);
      TabSet::new(&home, &places.set_start(start).path())
    }
  };
  let shown_project = match Route::parse(set.active_tab().path()) {
    Some(Route::Project(project, _)) => Some(project),
    _ => None,
  };
  Ok(Opening { set, shown_project })
}

/// The tabs the last session saved; `None` when it saved none or they cannot be read, which the log
/// says.
fn saved_tabs(workspace: &Workspace) -> Result<Option<SavedTabs>, StoreError> {
  let Some(text) = workspace.ui_state(TABS_KEY)? else {
    return Ok(None);
  };
  let saved = SavedTabs::decode(&text);
  if saved.is_none() {
    tracing::warn!("the saved tabs cannot be read; the window opens without them");
  }
  Ok(saved)
}

/// The saved tabs with every project they show opened; `None` when they hold no tab.
fn restore(workspace: &Workspace, places: &Places, saved: &SavedTabs) -> Result<Option<TabSet>, StoreError> {
  let projects = workspace.projects()?;
  // The start screen for a location that no longer applies: All projects, or the first-launch
  // screen while the workspace has no project.
  let start = if projects.is_empty() {
    Start::Welcome
  } else {
    Start::AllProjects
  };
  let start = places.set_start(start).path();
  let paths = saved
    .paths
    .iter()
    .map(|path| restored_path(workspace, places, &projects, path).unwrap_or_else(|| start.clone()))
    .collect();
  let saved = SavedTabs {
    paths,
    active: saved.active,
  };
  Ok(TabSet::restored(&Route::AllProjects.path(), &saved))
}

/// `path` as it can be shown now; `None` for a place that no longer exists.
fn restored_path(workspace: &Workspace, places: &Places, projects: &[RegisteredProject], path: &str) -> Option<String> {
  match Route::parse(path)? {
    Route::Project(project, _) => {
      if places.project(project).is_none() {
        let registered = projects.iter().find(|registered| registered.id == project)?;
        match workspace.open(&registered.repo_root) {
          Ok(open) => places.add_project(open),
          Err(error) => {
            tracing::warn!(%error, repo_root = %registered.repo_root.display(), "a saved tab's project cannot be opened");
            return None;
          }
        }
      }
      Some(path.to_owned())
    }
    route @ (Route::AllProjects | Route::AllUsage | Route::Setup | Route::Welcome) if places.applies(route) => {
      Some(path.to_owned())
    }
    Route::Start | Route::AllProjects | Route::AllUsage | Route::Setup | Route::Welcome => None,
  }
}
