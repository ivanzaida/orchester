//! Optional physical window origin, applied before the native window is created or shown.

use std::ffi::OsStr;

use thiserror::Error;

const WINDOW_X: &str = "ORCHESTER_APP_MCP_WINDOW_X";
const WINDOW_Y: &str = "ORCHESTER_APP_MCP_WINDOW_Y";

/// Signed physical coordinates; negative origins allow QA on displays left of the primary display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowPosition {
  x: i32,
  y: i32,
}

/// Invalid placement must stop startup rather than silently show on the default display.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum WindowPositionError {
  #[error("ORCHESTER_APP_MCP_WINDOW_X and ORCHESTER_APP_MCP_WINDOW_Y must be set together")]
  Incomplete,
  #[error("window coordinates must be signed 32-bit integers")]
  Invalid,
}

impl WindowPosition {
  /// Read the optional QA origin without changing the environment.
  pub fn from_environment() -> Result<Option<Self>, WindowPositionError> {
    let x = std::env::var_os(WINDOW_X);
    let y = std::env::var_os(WINDOW_Y);
    Self::parse(x.as_deref(), y.as_deref())
  }

  /// Validate both coordinates together. Neither set means normal OS placement.
  pub fn parse(x: Option<&OsStr>, y: Option<&OsStr>) -> Result<Option<Self>, WindowPositionError> {
    match (x, y) {
      (None, None) => Ok(None),
      (Some(x), Some(y)) => Ok(Some(Self {
        x: coordinate(x)?,
        y: coordinate(y)?,
      })),
      _ => Err(WindowPositionError::Incomplete),
    }
  }

  pub const fn coordinates(self) -> (i32, i32) {
    (self.x, self.y)
  }
}

fn coordinate(value: &OsStr) -> Result<i32, WindowPositionError> {
  value
    .to_str()
    .ok_or(WindowPositionError::Invalid)?
    .parse()
    .map_err(|_| WindowPositionError::Invalid)
}
