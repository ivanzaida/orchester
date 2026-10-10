//! Platforms other than Windows and macOS have an explicit unsupported result.

use std::io;

use super::{Entry, Info};

pub(super) struct Process {
  pub(super) info: Info,
}

fn unavailable() -> io::Error {
  io::Error::new(
    io::ErrorKind::Unsupported,
    "run process tools are supported on Windows and macOS only",
  )
}

pub(super) fn snapshot() -> io::Result<Vec<Entry>> {
  Err(unavailable())
}

impl Process {
  pub(super) fn open(_entry: Entry, _stopping: bool) -> io::Result<Option<Self>> {
    Err(unavailable())
  }
  pub(super) fn is_parent_of(&self, _entry: Entry) -> bool {
    false
  }
  pub(super) fn is_process(&self, entry: Entry) -> bool {
    entry.pid == self.info.pid
  }
  pub(super) fn alive(&self) -> io::Result<bool> {
    Err(unavailable())
  }
  pub(super) fn close(&self) -> io::Result<()> {
    Err(unavailable())
  }
  pub(super) fn end(&self) -> io::Result<()> {
    Err(unavailable())
  }
}
