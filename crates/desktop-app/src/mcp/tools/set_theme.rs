use lurq::{
  app::theme::Theme,
  mcp::{McpTool, Scope},
};
use serde_json::{Value, json};

use super::{ToolError, object, reply, string};
use crate::appearance::{Appearance, ThemeSetting};

const DESCRIPTION: &str = "Switch the Orchester window's colors and keep the choice in the app's settings: `theme` \
  is dark, light or system (the operating system's appearance when the app started). Answers the setting and the \
  mode the window shows. Errors are JSON with a stable `code`: missing_param, unexpected_param, not_a_string, \
  not_an_object, unknown_theme (with the known names), not_saved (nothing changed).";

pub fn tool(appearance: Appearance) -> McpTool {
  McpTool::new("set_theme")
    .description(DESCRIPTION)
    .scope(Scope::Interact)
    .input_schema(json!({
      "type": "object",
      "properties": {
        "theme": { "type": "string", "enum": ThemeSetting::ALL.map(ThemeSetting::name) },
      },
      "required": ["theme"],
      "additionalProperties": false,
    }))
    .handler(move |ctx, args| {
      let result = set_theme(&appearance, ctx.app.theme(), &args);
      // The roles resolve to the new colors on the next pass.
      ctx.tree.request_redraw();
      reply(result)
    })
}

/// Chooses the theme `args` name and gives `theme` its mode.
pub fn set_theme(appearance: &Appearance, theme: &Theme, args: &Value) -> Result<Value, ToolError> {
  let args = object(args)?;
  if let Some(param) = args.keys().find(|key| *key != "theme") {
    return Err(ToolError::UnexpectedParam {
      of: "set_theme",
      param: param.clone(),
    });
  }
  let name = string(&args, "theme")?.ok_or(ToolError::Missing { param: "theme" })?;
  let setting = ThemeSetting::from_name(name).ok_or_else(|| ToolError::UnknownTheme { name: name.to_owned() })?;
  let mode = appearance.set(theme, setting).map_err(|error| ToolError::NotSaved {
    reason: error.to_string(),
  })?;
  Ok(json!({ "setting": setting.name(), "mode": mode.name() }))
}
