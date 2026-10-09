use serde_json::{Value, json};

use crate::{appearance::ThemeSetting, routes::Screen};

/// Why an app tool refused. Agents read it as JSON with a stable `code` ([`ToolError::to_json`]).
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ToolError {
  #[error("the window is not ready yet; try again")]
  NotReady,
  #[error("the arguments must be a JSON object")]
  NotAnObject,
  #[error("`{param}` must be a string")]
  NotAString { param: String },
  #[error("`{param}` is required")]
  Missing { param: &'static str },
  #[error("unknown route `{name}`")]
  UnknownRoute { name: String },
  #[error("`{of}` takes no `{param}`")]
  UnexpectedParam { of: &'static str, param: String },
  #[error("`{param}`: {reason}")]
  InvalidParam { param: &'static str, reason: String },
  #[error("unknown tab `{name}`")]
  UnknownTab { name: String, known: Vec<&'static str> },
  #[error("unknown theme `{name}`")]
  UnknownTheme { name: String },
  #[error("{reason}")]
  NotSaved { reason: String },
}

impl ToolError {
  pub const fn code(&self) -> &'static str {
    match self {
      Self::NotReady => "not_ready",
      Self::NotAnObject => "not_an_object",
      Self::NotAString { .. } => "not_a_string",
      Self::Missing { .. } => "missing_param",
      Self::UnknownRoute { .. } => "unknown_route",
      Self::UnexpectedParam { .. } => "unexpected_param",
      Self::InvalidParam { .. } => "invalid_param",
      Self::UnknownTab { .. } => "unknown_tab",
      Self::UnknownTheme { .. } => "unknown_theme",
      Self::NotSaved { .. } => "not_saved",
    }
  }

  /// `{"code", "message"}`, with the names that are known where a name was not.
  pub fn to_json(&self) -> Value {
    let known: Option<Vec<&str>> = match self {
      Self::UnknownRoute { .. } => Some(Screen::ALL.into_iter().map(Screen::name).collect()),
      Self::UnknownTab { known, .. } => Some(known.clone()),
      Self::UnknownTheme { .. } => Some(ThemeSetting::ALL.into_iter().map(ThemeSetting::name).collect()),
      _ => None,
    };
    let mut error = json!({ "code": self.code(), "message": self.to_string() });
    if let Some(known) = known {
      error["known"] = json!(known);
    }
    error
  }
}
