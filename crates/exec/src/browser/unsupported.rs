//! Platforms other than Windows and macOS have no known place to find the browser and know no browser's program: it
//! is unavailable, saying so.

use std::path::{Path, PathBuf};

use super::{BrowserProgram, BrowserUnavailable};

pub(super) fn find() -> Result<BrowserProgram, BrowserUnavailable> {
  Err(BrowserUnavailable::new(
    "the browser for agents is not supported on this platform yet",
  ))
}

pub(super) fn find_in(_: &[PathBuf]) -> Result<BrowserProgram, BrowserUnavailable> {
  find()
}

pub(super) fn program_in(path: &Path) -> PathBuf {
  path.to_path_buf()
}

pub(super) fn named(_: &Path) -> Option<&'static str> {
  None
}
