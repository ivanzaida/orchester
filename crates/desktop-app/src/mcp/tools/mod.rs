//! The app's own MCP tools, served beside lurq's `lurq_*`: `app_state` reads where the window is,
//! `navigate` takes the active tab somewhere, `set_theme` switches the colors, and in debug builds
//! `hold_reads` holds the project feeds' reads for QA. They run on the event-loop thread, with the tree
//! and the app, like every other change to the window.

mod app_state;
mod error;
mod hold_reads;
mod navigate;
mod set_theme;

use lurq::mcp::McpTool;
use serde_json::{Map, Value, json};

pub use self::{
  app_state::app_state, error::ToolError, hold_reads::hold_reads, navigate::navigate, set_theme::set_theme,
};
use super::{McpSettings, WindowLink};
use crate::{
  appearance::Appearance,
  project::hold::ReadHold,
  routes::{ProjectPage, Route},
};

/// Every app tool, for lurq's MCP configuration; `hold_reads` only in debug builds of a started session,
/// whose reads `hold` holds.
pub fn all(link: &WindowLink, appearance: &Appearance, settings: McpSettings, hold: Option<ReadHold>) -> Vec<McpTool> {
  let mut tools = vec![
    app_state::tool(link.clone(), appearance.clone(), settings),
    navigate::tool(link.clone()),
    set_theme::tool(appearance.clone()),
  ];
  if cfg!(debug_assertions)
    && let Some(hold) = hold
  {
    tools.push(hold_reads::tool(hold));
  }
  tools
}

/// What lurq answers for a tool: its value, or its error as JSON text.
fn reply(result: Result<Value, ToolError>) -> Result<Value, String> {
  result.map_err(|error| error.to_json().to_string())
}

/// A route as the tools show it: its screen's name, its path and the parameters `navigate` takes, a tab
/// always and the others when the route has them.
pub fn route_json(route: &Route) -> Value {
  let mut params = Map::new();
  if let Route::Project(project, page) = route {
    params.insert("project".to_owned(), json!(project.to_string()));
    page_params(page, &mut params);
  }
  json!({ "name": route.screen().name(), "path": route.path(), "params": params })
}

fn page_params(page: &ProjectPage, params: &mut Map<String, Value>) {
  let mut insert = |name: &str, value: &str| {
    params.insert(name.to_owned(), json!(value));
  };
  match page {
    ProjectPage::Overview(tab) => insert("tab", tab.name()),
    ProjectPage::Tasks(list) => {
      insert("tab", list.tab().name());
      if let Some(text) = list.search() {
        insert("q", text);
      }
      if let Some(key) = list.highlight() {
        insert("highlight", &key.to_string());
      }
    }
    ProjectPage::EditTask(key) => insert("key", &key.to_string()),
    ProjectPage::Connections(view) => {
      insert("tab", view.tab().name());
      if let Some(connection) = view.selected() {
        insert("selected", &connection.to_string());
      }
    }
    ProjectPage::NewConnection(kind) => insert("kind", kind.name()),
    ProjectPage::EditConnection(connection) => insert("connection", &connection.to_string()),
    ProjectPage::Task(key, view) => {
      insert("key", &key.to_string());
      insert("tab", view.tab().name());
      if let Some(file) = view.file() {
        insert("file", file.path());
        insert("from", file.source().name());
      }
    }
    _ => {}
  }
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
