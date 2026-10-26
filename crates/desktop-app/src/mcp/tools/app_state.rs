use lurq::{
  app::Tree,
  mcp::{McpTool, Scope},
};
use orchester_workspace::runner::state::{RunnerFailure, RunnerState};
use serde_json::{Value, json};

use super::{McpSettings, WindowLink, reply, route_json};
use crate::{appearance::Appearance, project::Projects, root::WINDOW_ID, routes::Route};

const DESCRIPTION: &str = "Where the Orchester window is: the active tab's route (name, path, params) and whether \
  it can go back or forward, every open tab with its route and, for a project's screen, the project (its name, \
  folder, whether it opened: open, unknown, folder_missing, unreadable, failed, or null while it opens, and its \
  runner's state with the cause of a failure), the theme (setting dark/light/system and the mode it shows), \
  whether agents may control the window and what decided it (build, stored, environment), and the window's size \
  in logical pixels with its scale factor (null before the first layout). A window that could not open its data \
  has no tabs: it reports the startup screen it shows instead (already_open, newer_data, update_failed, \
  unreadable, not_writable, program_folder_unknown).";

pub fn tool(link: WindowLink, appearance: Appearance, settings: McpSettings) -> McpTool {
  McpTool::new("app_state")
    .description(DESCRIPTION)
    .scope(Scope::Observe)
    .read_only()
    .input_schema(json!({ "type": "object", "properties": {}, "additionalProperties": false }))
    .handler(move |ctx, _args| reply(app_state(&link, &appearance, settings, ctx.tree)))
}

/// The state `app_state` reports for the window `tree` shows.
pub fn app_state(
  link: &WindowLink,
  appearance: &Appearance,
  settings: McpSettings,
  tree: &mut Tree,
) -> Result<Value, super::ToolError> {
  let mut state = json!({
    "theme": { "setting": appearance.setting().name(), "mode": appearance.mode().name() },
    "mcp": { "window_control": settings.window_control(), "source": settings.source().name() },
    "window": window(tree),
  });
  if let Some(screen) = link.startup() {
    state["startup"] = json!(screen);
    return Ok(state);
  }
  let (navigation, projects) = (link.navigation()?, link.projects()?);
  let tabs = navigation.tabs_now();
  let active = tabs.active();
  let open: Vec<Value> = tabs
    .tabs()
    .iter()
    .map(|tab| {
      let mut entry = json!({
        "id": tab.id().get(),
        "active": tab.id() == active.id(),
        "route": route_json(tab.route()),
      });
      if let Some(project) = project_json(&projects, tab.route()) {
        entry["project"] = project;
      }
      entry
    })
    .collect();
  state["route"] = route_json(active.route());
  state["history"] = json!({ "can_back": tabs.can_back(), "can_forward": active.can_forward() });
  state["tabs"] = json!(open);
  Ok(state)
}

/// The project `route` shows, as far as the window knows it; `None` outside a project.
fn project_json(projects: &Projects, route: &Route) -> Option<Value> {
  let id = route.project()?;
  let Some(state) = projects.state_now(id) else {
    return Some(json!({ "state": null }));
  };
  let runner = state
    .open()
    .and_then(|_| projects.feed_now(id))
    .and_then(|feed| feed.status.get_untracked())
    .map(|status| runner_json(status.state()));
  Some(json!({
    "name": state.name().map(|name| name.as_str()),
    "path": state.repo_root().map(|root| root.display().to_string()),
    "state": state.kind(),
    "runner": runner,
  }))
}

fn runner_json(state: &RunnerState) -> Value {
  let (name, cause) = match state {
    RunnerState::Starting => ("starting", None),
    RunnerState::Online => ("online", None),
    RunnerState::Reconnecting { cause, .. } => ("reconnecting", Some(cause)),
    RunnerState::Stopping => ("stopping", None),
    RunnerState::Stopped => ("stopped", None),
    RunnerState::Failed { cause, .. } => ("failed", Some(cause)),
  };
  json!({ "state": name, "cause": cause.map(failure_name) })
}

const fn failure_name(failure: &RunnerFailure) -> &'static str {
  match failure {
    RunnerFailure::MissingExecutable { .. } => "missing_executable",
    RunnerFailure::AlreadyServed => "already_served",
    RunnerFailure::StartFailed => "start_failed",
    RunnerFailure::Lost => "lost",
  }
}

fn window(tree: &mut Tree) -> Value {
  let Some(bounds) = tree.get_element_by_id_mut(WINDOW_ID).and_then(|window| window.bounds()) else {
    return Value::Null;
  };
  json!({ "width": bounds.width, "height": bounds.height, "scale_factor": tree.scale_factor() })
}
