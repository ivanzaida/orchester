//! Platforms other than Windows and macOS have no known place to find the browser: it is unavailable, saying so.

use super::{BrowserProgram, BrowserUnavailable};

pub(super) fn find() -> Result<BrowserProgram, BrowserUnavailable> {
  Err(BrowserUnavailable::new(
    "the browser for agents is not supported on this platform yet",
  ))
}
