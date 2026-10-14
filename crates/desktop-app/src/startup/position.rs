//! Where the window opens when QA asks for a place (`tools/qa-app.ps1 -WindowX -WindowY`), for example
//! on a display other than the person's main one. Without the variable the system places the window, as
//! always; the place is never stored. A QA copy or any other build that is not the installed app refuses a
//! malformed value and does not start, so a QA run never gets a window elsewhere than it asked; the installed
//! app ignores a malformed value with a log line, so a stray variable never keeps it from starting.

use std::ffi::OsStr;

use orchester_workspace::build::Build;

/// `X,Y`: the window's top-left corner in physical pixels of the virtual desktop; either may be negative,
/// for a display left of or above the main one.
pub const POSITION_VARIABLE: &str = "ORCHESTER_APP_WINDOW_POSITION";

/// The top-left corner the window opens at, in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowPosition {
  pub x: i32,
  pub y: i32,
}

/// Why the requested place was refused; a build that is not the installed app does not open a window then.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PositionError {
  #[error("{POSITION_VARIABLE} is not text")]
  NotText,
  #[error("{POSITION_VARIABLE} is `{0}`, not the pair X,Y")]
  NotAPair(String),
  #[error("{POSITION_VARIABLE} has `{value}` for {axis}, not a whole number of pixels")]
  NotANumber { axis: char, value: String },
}

impl WindowPosition {
  /// The place this process was asked for; `None` when it was not ([`Self::for_build`]).
  pub fn from_environment() -> Result<Option<Self>, PositionError> {
    Self::for_build(Build::this(), std::env::var_os(POSITION_VARIABLE).as_deref())
  }

  /// The place `build` takes from a value of [`POSITION_VARIABLE`]: the installed app logs a malformed value and
  /// lets the system place the window; every other build refuses it.
  pub fn for_build(build: Build, value: Option<&OsStr>) -> Result<Option<Self>, PositionError> {
    match (build, Self::of_variable(value)) {
      (Build::Installed, Err(error)) => {
        tracing::warn!(%error, "the requested window position is ignored; the system places the window");
        Ok(None)
      }
      (_, read) => read,
    }
  }

  /// The place a value of `POSITION_VARIABLE` asks for; `None` when the variable is not set.
  pub fn of_variable(value: Option<&OsStr>) -> Result<Option<Self>, PositionError> {
    value
      .map(|value| value.to_str().ok_or(PositionError::NotText).and_then(Self::parse))
      .transpose()
  }

  /// `X,Y`, both signed whole numbers, nothing else.
  pub fn parse(text: &str) -> Result<Self, PositionError> {
    let Some((x, y)) = text.split_once(',') else {
      return Err(PositionError::NotAPair(text.to_owned()));
    };
    if x.is_empty() || y.is_empty() || y.contains(',') {
      return Err(PositionError::NotAPair(text.to_owned()));
    }
    Ok(Self {
      x: coordinate('X', x)?,
      y: coordinate('Y', y)?,
    })
  }
}

fn coordinate(axis: char, value: &str) -> Result<i32, PositionError> {
  value.parse().map_err(|_| PositionError::NotANumber {
    axis,
    value: value.to_owned(),
  })
}
