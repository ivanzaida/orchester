//! What agents may do through the app's MCP server. Looking is always allowed: screenshots, the element
//! tree and logs (lurq's Observe scope) and `app_state`. Window control is changing the window: clicking,
//! typing, resizing (lurq's Interact scope), `navigate` and `set_theme`. It is on by default in debug
//! builds, which design verification drives, and off by default in release builds; the app's settings
//! (`orchester.db`) decide once they hold a value, and `ORCHESTER_APP_MCP_WINDOW_CONTROL` (`on`/`off`)
//! overrides both for one run.
//!
//! lurq's Navigate scope (`lurq_navigate`) is never granted: it would move the router past the tabs'
//! histories; `navigate` goes through them.

use lurq::mcp::Scope;

/// The environment variable that turns window control on or off for one run.
pub const WINDOW_CONTROL_VARIABLE: &str = "ORCHESTER_APP_MCP_WINDOW_CONTROL";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct McpSettings {
  window_control: bool,
}

impl McpSettings {
  /// Window control in debug builds only.
  pub const fn of_build() -> Self {
    Self {
      window_control: cfg!(debug_assertions),
    }
  }

  pub const fn with_window_control(window_control: bool) -> Self {
    Self { window_control }
  }

  /// These settings with the stored value, when the app's settings hold one.
  pub const fn with_stored(self, stored: Option<bool>) -> Self {
    match stored {
      Some(window_control) => Self::with_window_control(window_control),
      None => self,
    }
  }

  /// These settings with what the environment says for this run. A value that is not Unicode is
  /// logged and ignored.
  pub fn with_environment(self) -> Self {
    self.overridden_by(|name| match std::env::var(name) {
      Ok(value) => Some(value),
      Err(std::env::VarError::NotPresent) => None,
      Err(std::env::VarError::NotUnicode(_)) => {
        tracing::warn!(variable = name, "not Unicode; window control stays as set");
        None
      }
    })
  }

  /// These settings with the value `variable` finds for [`WINDOW_CONTROL_VARIABLE`] applied on top. A
  /// value that is not `on`/`off` (or `1`/`0`, `true`/`false`) keeps the setting and is logged.
  pub fn overridden_by(self, variable: impl Fn(&str) -> Option<String>) -> Self {
    let Some(value) = variable(WINDOW_CONTROL_VARIABLE) else {
      return self;
    };
    match value.as_str() {
      "on" | "1" | "true" => Self::with_window_control(true),
      "off" | "0" | "false" => Self::with_window_control(false),
      _ => {
        tracing::warn!(variable = WINDOW_CONTROL_VARIABLE, %value, "not on or off; window control stays as built");
        self
      }
    }
  }

  pub const fn window_control(self) -> bool {
    self.window_control
  }

  /// lurq's scopes for these settings; the app's tools use the same two.
  pub fn scopes(self) -> Vec<Scope> {
    if self.window_control {
      vec![Scope::Observe, Scope::Interact]
    } else {
      vec![Scope::Observe]
    }
  }
}
