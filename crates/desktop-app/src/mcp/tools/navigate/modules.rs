//! What navigate makes of `route` modules: the list on a tab, New module, a module's page on a tab, or Edit module.

use orchester_core::id::ModuleId;
use serde_json::{Map, Value};

use super::{ToolError, string, tab};
use crate::routes::{ModuleListTab, ModuleTab, ProjectPage};

/// The Modules list on `tab`; with `module` new, New module; with a module's id, its page on `tab`, or with `view`
/// edit, Edit module.
pub(super) fn module_page(args: &Map<String, Value>) -> Result<ProjectPage, ToolError> {
  let view = string(args, "view")?;
  let Some(module) = string(args, "module")? else {
    let known = ModuleListTab::ALL.map(ModuleListTab::name);
    return Ok(ProjectPage::Modules(tab(args, ModuleListTab::from_name, &known)?));
  };
  if module == "new" {
    return Ok(ProjectPage::NewModule);
  }
  let id = module.parse::<ModuleId>().map_err(|error| ToolError::InvalidParam {
    param: "module",
    reason: error.to_string(),
  })?;
  match view {
    None => {
      let known = ModuleTab::ALL.map(ModuleTab::name);
      Ok(ProjectPage::Module(id, tab(args, ModuleTab::from_name, &known)?))
    }
    Some("edit") => Ok(ProjectPage::EditModule(id)),
    Some(other) => Err(ToolError::InvalidParam {
      param: "view",
      reason: format!("`{other}` names no view of a module: its one view is edit"),
    }),
  }
}
