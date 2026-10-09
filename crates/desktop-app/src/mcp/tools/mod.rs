//! The app's own MCP tools, served beside lurq's `lurq_*`: `app_state` reads where the window is,
//! `navigate` takes the active tab somewhere, `set_theme` switches the colors. They run on the
//! event-loop thread, with the tree and the app, like every other change to the window.

mod app_state;
mod error;
mod navigate;
mod set_theme;

use lurq::mcp::McpTool;
use serde_json::{Map, Value, json};

pub use self::{app_state::app_state, error::ToolError, navigate::navigate, set_theme::set_theme};
use super::WindowLink;
use crate::{
  appearance::Appearance,
  routes::{ProjectPage, Route},
};

/// Every app tool, for lurq's MCP configuration.
pub fn all(link: &WindowLink, appearance: &Appearance) -> [McpTool; 3] {
  [
    app_state::tool(link.clone(), appearance.clone()),
    navigate::tool(link.clone()),
    set_theme::tool(appearance.clone()),
  ]
}

/// What lurq answers for a tool: its value, or its error as JSON text.
fn reply(result: Result<Value, ToolError>) -> Result<Value, String> {
  result.map_err(|error| error.to_json().to_string())
}

/// A route as the tools show it: its screen's name, its path and the parameters `navigate` takes.
pub fn route_json(route: &Route) -> Value {
  let mut params = Map::new();
  if let Some(project) = route.project() {
    params.insert("project".to_owned(), json!(project.to_string()));
  }
  if let Route::Project(_, ProjectPage::Task(key, tab)) = route {
    params.insert("key".to_owned(), json!(key.to_string()));
    params.insert("tab".to_owned(), json!(tab.name()));
  }
  json!({ "name": route.screen().name(), "path": route.path(), "params": params })
}

/// The string argument `name`, if given.
fn string<'a>(args: &'a Map<String, Value>, name: &str) -> Result<Option<&'a str>, ToolError> {
  match args.get(name) {
    None => Ok(None),
    Some(Value::String(value)) => Ok(Some(value)),
    Some(_) => Err(ToolError::NotAString { param: name.to_owned() }),
  }
}

/// The arguments as an object; no arguments at all are an empty one.
fn object(args: &Value) -> Result<Map<String, Value>, ToolError> {
  match args {
    Value::Null => Ok(Map::new()),
    Value::Object(args) => Ok(args.clone()),
    _ => Err(ToolError::NotAnObject),
  }
}
