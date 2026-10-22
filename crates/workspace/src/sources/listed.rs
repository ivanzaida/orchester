//! A listed source as a screen shows it (`Source row` `Q5DCn`): what it names and whether runs can read it now. A
//! source that is missing or cannot be read stays listed: runs start without it, and a read there asks.

use std::{fs, io};

use orchester_core::source::SourcePath;

use super::check::Kind;
use crate::messages::Message;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
  Available,
  Missing,
  Unreadable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
  pub path: SourcePath,
  /// What it names; `None` while nothing is there.
  pub kind: Option<Kind>,
  pub state: State,
}

impl Listed {
  /// `path` as it is on disk now. Reads the file system: call it off the UI thread.
  pub fn inspect(path: &SourcePath) -> Self {
    let (kind, state) = match fs::metadata(path.path()) {
      Ok(found) if found.is_dir() => (Some(Kind::Folder), readable(fs::read_dir(path.path()).map(drop))),
      Ok(_) => (Some(Kind::File), readable(fs::File::open(path.path()).map(drop))),
      Err(error) if error.kind() == io::ErrorKind::NotFound => (None, State::Missing),
      Err(_) => (None, State::Unreadable),
    };
    Self {
      path: path.clone(),
      kind,
      state,
    }
  }

  /// What a run does about it while it is missing or cannot be read; `None` while it is available.
  pub fn problem(&self) -> Option<Message> {
    match self.state {
      State::Available => None,
      State::Missing => Some(Message::new("sources.listed.missing")),
      State::Unreadable => Some(Message::new("sources.listed.unreadable")),
    }
  }
}

fn readable(opened: io::Result<()>) -> State {
  match opened {
    Ok(()) => State::Available,
    Err(error) if error.kind() == io::ErrorKind::NotFound => State::Missing,
    Err(_) => State::Unreadable,
  }
}

/// Each of `paths` as it is on disk now.
pub fn inspect_all(paths: &[SourcePath]) -> Vec<Listed> {
  paths.iter().map(Listed::inspect).collect()
}
