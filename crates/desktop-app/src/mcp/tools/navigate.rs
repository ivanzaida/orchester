use lurq::mcp::{McpTool, Scope};
use orchester_core::{
  id::ProjectId,
  project::{TaskKey, TaskKeyError},
};
use serde_json::{Map, Value, json};

use super::{ToolError, WindowLink, object, reply, route_json, string};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, Screen, TaskTab},
};

const DESCRIPTION: &str = "Take the active tab of the Orchester window to a route, as a new entry of its history   (the mouse's back button, or Alt+Left, Cmd+[ on macOS, returns). `route` is the screen's name: all_projects and   settings take no other argument; a task takes `project`, `key` and optionally `tab` (conversation, summary,   changes); every other screen takes `project`, the project's id (a UUID, as app_state reports it). Answers the   route the tab shows. Errors are JSON with a stable `code`: unknown_route (with the known names), missing_param,   unexpected_param, invalid_param, unknown_tab, not_a_string, not_an_object, not_ready (the window is starting).";

pub fn tool(link: WindowLink) -> McpTool {
  McpTool::new("navigate")
    .description(DESCRIPTION)
    .scope(Scope::Interact)
    .input_schema(schema())
    .handler(move |_ctx, args| reply(link.navigation().and_then(|navigation| navigate(&navigation, &args))))
}

fn schema() -> Value {
  let names: Vec<&str> = Screen::ALL.into_iter().map(Screen::name).collect();
  let tabs: Vec<&str> = TaskTab::ALL.into_iter().map(TaskTab::name).collect();
  json!({
    "type": "object",
    "properties": {
      "route": { "type": "string", "enum": names },
      "project": { "type": "string", "description": "The project's id (a UUID), for every screen of a project." },
      "key": { "type": "string", "description": "The task's key, e.g. KON-12, for `task`." },
      "tab": { "type": "string", "enum": tabs, "description": "The task's tab; conversation when left out." },
    },
    "required": ["route"],
    "additionalProperties": false,
  })
}

/// Takes the active tab to the route `args` name and answers the route it shows.
pub fn navigate(navigation: &Navigation, args: &Value) -> Result<Value, ToolError> {
  let route = target(&object(args)?)?;
  navigation.go(route);
  Ok(json!({ "route": route_json(&navigation.route_now()) }))
}

/// The route `args` name: `route`, and the parameters its screen takes.
fn target(args: &Map<String, Value>) -> Result<Route, ToolError> {
  let name = string(args, "route")?.ok_or(ToolError::Missing { param: "route" })?;
  let screen = Screen::from_name(name).ok_or_else(|| ToolError::UnknownRoute { name: name.to_owned() })?;
  let takes = params(screen);
  if let Some(param) = args
    .keys()
    .find(|key| *key != "route" && !takes.contains(&key.as_str()))
  {
    return Err(ToolError::UnexpectedParam {
      of: screen.name(),
      param: param.clone(),
    });
  }
  Ok(match (screen, ProjectPage::of(screen)) {
    (Screen::AllProjects, _) => Route::AllProjects,
    (Screen::Settings, _) => Route::Settings,
    (_, Some(page)) => Route::Project(project(args)?, page),
    (_, None) => Route::Project(project(args)?, ProjectPage::Task(key(args)?, tab(args)?)),
  })
}

/// The parameters besides `route` that `screen` takes.
const fn params(screen: Screen) -> &'static [&'static str] {
  match screen {
    Screen::AllProjects | Screen::Settings => &[],
    Screen::Task => &["project", "key", "tab"],
    _ => &["project"],
  }
}

fn project(args: &Map<String, Value>) -> Result<ProjectId, ToolError> {
  let value = string(args, "project")?.ok_or(ToolError::Missing { param: "project" })?;
  value.parse().map_err(|error: uuid::Error| ToolError::InvalidParam {
    param: "project",
    reason: error.to_string(),
  })
}

fn key(args: &Map<String, Value>) -> Result<TaskKey, ToolError> {
  let value = string(args, "key")?.ok_or(ToolError::Missing { param: "key" })?;
  value.parse().map_err(|error: TaskKeyError| ToolError::InvalidParam {
    param: "key",
    reason: error.to_string(),
  })
}

fn tab(args: &Map<String, Value>) -> Result<TaskTab, ToolError> {
  let Some(name) = string(args, "tab")? else {
    return Ok(TaskTab::default());
  };
  TaskTab::from_name(name).ok_or_else(|| ToolError::UnknownTab { name: name.to_owned() })
}
