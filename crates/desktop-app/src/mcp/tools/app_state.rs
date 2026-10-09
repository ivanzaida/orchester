use lurq::{
  app::Tree,
  mcp::{McpTool, Scope},
};
use serde_json::{Value, json};

use super::{McpSettings, WindowLink, reply, route_json};
use crate::{appearance::Appearance, navigation::Navigation, root::WINDOW_ID};

const DESCRIPTION: &str = "Where the Orchester window is: the active tab's route (name, path, params) and whether \
  it can go back or forward, every open tab with its route, the theme (setting dark/light/system and the mode it \
  shows), whether agents may control the window and what decided it (build, stored, environment), and the \
  window's size in logical pixels with its scale factor (null before the first layout).";

pub fn tool(link: WindowLink, appearance: Appearance, settings: McpSettings) -> McpTool {
  McpTool::new("app_state")
    .description(DESCRIPTION)
    .scope(Scope::Observe)
    .read_only()
    .input_schema(json!({ "type": "object", "properties": {}, "additionalProperties": false }))
    .handler(move |ctx, _args| {
      reply(
        link
          .navigation()
          .map(|navigation| app_state(&navigation, &appearance, settings, ctx.tree)),
      )
    })
}

/// The state `app_state` reports for the window `tree` shows.
pub fn app_state(navigation: &Navigation, appearance: &Appearance, settings: McpSettings, tree: &mut Tree) -> Value {
  let tabs = navigation.tabs_now();
  let active = tabs.active();
  let open: Vec<Value> = tabs
    .tabs()
    .iter()
    .map(|tab| {
      json!({
        "id": tab.id().get(),
        "active": tab.id() == active.id(),
        "route": route_json(tab.route()),
      })
    })
    .collect();
  json!({
    "route": route_json(active.route()),
    "history": { "can_back": active.can_back(), "can_forward": active.can_forward() },
    "tabs": open,
    "theme": { "setting": appearance.setting().name(), "mode": appearance.mode().name() },
    "mcp": { "window_control": settings.window_control(), "source": settings.source().name() },
    "window": window(tree),
  })
}

fn window(tree: &mut Tree) -> Value {
  let Some(bounds) = tree.get_element_by_id_mut(WINDOW_ID).and_then(|window| window.bounds()) else {
    return Value::Null;
  };
  json!({ "width": bounds.width, "height": bounds.height, "scale_factor": tree.scale_factor() })
}
