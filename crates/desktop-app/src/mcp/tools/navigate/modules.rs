//! What navigate makes of `route` modules: the list on a tab, New module, a module's page on a tab, or Edit module;
//! of `route` new_task with `module`: New task in that module; and of `route` agents with `agent`: a profile's page or
//! a new profile. A `tab` or `view` where it names nothing is refused, not left out.

use orchester_core::id::{ModuleId, ResourceId};
use serde_json::{Map, Value};

use super::{ToolError, string, tab};
use crate::routes::{AgentTab, ModuleListTab, ModuleTab, ProjectPage};

/// The profiles' list; with `agent` new, a new profile; with a profile's id, its page.
pub(super) fn agent_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  match string(args, "agent")? {
    None => Ok(ProjectPage::Agents),
    Some("new") => Ok(ProjectPage::NewAgent(AgentTab::default())),
    Some(agent) => agent
      .parse::<ResourceId>()
      .map(|id| ProjectPage::Agent(id, AgentTab::default()))
      .map_err(|error| ToolError::InvalidParam {
        param: "agent",
        reason: error.to_string(),
      }),
  }
}

/// The Modules list on `tab`; with `module` new or `view` new, New module; with a module's id, its page on `tab`, or
/// with `view` edit, Edit module.
pub(super) fn module_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let view = string(args, "view")?;
  let Some(module) = string(args, "module")? else {
    return match view {
      None => {
        let known = ModuleListTab::ALL.map(ModuleListTab::name);
        Ok(ProjectPage::Modules(tab(args, ModuleListTab::from_name, &known)?))
      }
      Some("new") => no_tab(args, "New module has no tabs").map(|()| ProjectPage::NewModule),
      Some(other) => Err(ToolError::InvalidParam {
        param: "view",
        reason: format!("`{other}` needs `module`: without one the modules' one view is new, for New module"),
      }),
    };
  };
  if module == "new" {
    if view.is_some() {
      return Err(ToolError::InvalidParam {
        param: "view",
        reason: "New module has no views".to_owned(),
      });
    }
    return no_tab(args, "New module has no tabs").map(|()| ProjectPage::NewModule);
  }
  let id = module_id(module)?;
  match view {
    None => {
      let known = ModuleTab::ALL.map(ModuleTab::name);
      Ok(ProjectPage::Module(id, tab(args, ModuleTab::from_name, &known)?))
    }
    Some("edit") => no_tab(args, "Edit module has no tabs").map(|()| ProjectPage::EditModule(id)),
    Some(other) => Err(ToolError::InvalidParam {
      param: "view",
      reason: format!("`{other}` names no view of a module: its one view is edit"),
    }),
  }
}

/// New task, or with `module` a module's id, New task in that module.
pub(super) fn new_task_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  match string(args, "module")? {
    None => Ok(ProjectPage::NewTask),
    Some(module) => module_id(module).map(ProjectPage::NewTaskInModule),
  }
}

fn module_id(module: &str) -> Result<ModuleId, ToolError> {
  module.parse::<ModuleId>().map_err(|error| ToolError::InvalidParam {
    param: "module",
    reason: error.to_string(),
  })
}

/// Refuses a `tab` on a page that has none, saying `why`.
fn no_tab(args: &Map<String, Value>, why: &str) -> Result<(), ToolError> {
  if args.contains_key("tab") {
    return Err(ToolError::InvalidParam {
      param: "tab",
      reason: why.to_owned(),
    });
  }
  Ok(())
}
