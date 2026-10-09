use orchester_core::project::TaskKeyError;

/// Why a path names no place in the window. The window shows it in place of a screen.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum RouteError {
  #[error("no screen has this path")]
  NoScreen,
  #[error("`{value}` is not a project id")]
  ProjectId { value: String },
  #[error("`{value}` is not a task key")]
  TaskKey {
    value: String,
    #[source]
    source: TaskKeyError,
  },
  /// A query parameter the screen does not take, or takes once and is given again.
  #[error("the screen does not take `{param}` here")]
  UnexpectedParam { param: String },
  /// A query value the screen cannot show: an unknown tab, a malformed escape, an empty file path.
  #[error("`{value}` is not a value of `{param}`")]
  InvalidParam { param: String, value: String },
}
