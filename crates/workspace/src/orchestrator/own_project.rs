//! The Orchestrator chat acts only in its own project ([`super::tools`]): a change is given the chat's project by its
//! id, never by a name, and one that names another project is refused before it reaches the tool.

use rmcp::model::JsonObject;
use serde_json::Value;

use super::tools::ChatScope;
use crate::{app_mcp::ToolError, messages::Message};

/// Holds a change to the chat's project: one that names it, by its id or its name, or names none, is given it by its
/// id; one that names another project is refused.
pub(super) fn in_own_project(scope: &ChatScope, arguments: &mut JsonObject) -> Result<(), ToolError> {
  let own = match arguments.get("project") {
    None => true,
    Some(Value::String(named)) => names_own_project(scope, named),
    Some(_) => false,
  };
  if !own {
    return Err(ToolError::refused(
      "other_project",
      vec![Message::new("orchestrator.tools.otherProject")],
    ));
  }
  arguments.insert("project".to_owned(), Value::String(scope.project.to_string()));
  Ok(())
}

/// A read that names the chat's project, by its id or its name, is given it by its id: the name may be another
/// project's by now. A read of another project, or of every project, stays as it is.
pub(super) fn own_project_by_id(scope: &ChatScope, arguments: &mut JsonObject) {
  if let Some(Value::String(named)) = arguments.get("project")
    && names_own_project(scope, named)
  {
    arguments.insert("project".to_owned(), Value::String(scope.project.to_string()));
  }
}

/// Whether `named` names the chat's project, by its id or its name.
fn names_own_project(scope: &ChatScope, named: &str) -> bool {
  named == scope.project.to_string() || named.eq_ignore_ascii_case(&scope.name)
}
