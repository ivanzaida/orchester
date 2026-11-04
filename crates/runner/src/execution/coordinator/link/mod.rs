//! The coordinator's local side of the coordinator link (`super::super::executor::link`): it answers the executor's
//! requests by reading the project database, as the executor did before it asked. Each area of the link answers in a
//! module of its own.

mod approvals;
mod base_notices;
mod environment;
mod memory;
mod messages;
mod questions;
mod resources;
mod settings;
mod usage;

use std::{
  ops::Deref,
  path::{Path, PathBuf},
};

use orchester_store::{ProjectDb, ReadOnlyProjectDb, StoreError};

use super::Scheduler;
use crate::execution::executor::link::{CoordinatorLink, LinkError};

/// The coordinator link of an executor in the coordinator's own process, over the project database `D` holds.
pub(in crate::execution) struct LocalLink<D>(D);

impl<D: Deref<Target = ProjectDb>> LocalLink<D> {
  /// The link over `db`.
  pub(in crate::execution) fn over(db: D) -> Self {
    Self(db)
  }
}

/// A connection of a run's task's own that only reads the project database, and the project root it is in.
pub(in crate::execution) struct RunReader {
  db: ReadOnlyProjectDb,
  root: PathBuf,
}

impl Deref for RunReader {
  type Target = ProjectDb;

  fn deref(&self) -> &ProjectDb {
    &self.db
  }
}

impl LocalLink<RunReader> {
  /// The link of a run's task, over a connection of its own that only reads the project database at `root`.
  pub(in crate::execution) fn open(root: &Path) -> Result<Self, StoreError> {
    let db = ProjectDb::open_read_only(root)?;
    Ok(Self(RunReader {
      db,
      root: root.to_path_buf(),
    }))
  }
}

impl CoordinatorLink for LocalLink<RunReader> {
  fn another(&self) -> Result<Box<dyn CoordinatorLink + Send>, LinkError> {
    let link = Self::open(&self.0.root).map_err(|error| read_failed(&error))?;
    Ok(Box::new(link))
  }
}

/// Opens local links of the project at a root, each over a read-only connection of its own ([`LocalLink::open`]).
pub struct LocalLinks {
  root: PathBuf,
}

impl LocalLinks {
  /// The links of the project whose database is at `root`.
  pub fn new(root: &Path) -> Self {
    Self {
      root: root.to_path_buf(),
    }
  }

  /// A new link to the coordinator; the link opens more of its own with [`CoordinatorLink::another`].
  pub fn open(&self) -> Result<Box<dyn CoordinatorLink + Send>, LinkError> {
    let link = LocalLink::open(&self.root).map_err(|error| read_failed(&error))?;
    Ok(Box::new(link))
  }
}

impl Scheduler {
  /// A new link of the project's executor to its coordinator, which a run's task and the executor's work outside one
  /// ask the project's data through: here the local link, over a read-only connection of its own, and over the
  /// channel that serializes every message in the runner's tests of the boundary (`super::channel`). The one place
  /// the executor is handed its coordinator's link.
  pub(in crate::execution) fn open_link(&self) -> Result<Box<dyn CoordinatorLink + Send>, LinkError> {
    let link = LocalLinks::new(&self.inner.project_root).open()?;
    #[cfg(feature = "test-agent")]
    if let Some(calls) = &self.inner.channel {
      return Ok(Box::new(super::channel::ChannelLink::over(link, calls.clone())));
    }
    Ok(link)
  }
}

/// What reading the project database failed with, as the executor gets it: its text, and the kind of failure the
/// store makes of it.
fn read_failed(error: &StoreError) -> LinkError {
  LinkError {
    detail: error.to_string(),
    failure: error.runner_failure(),
  }
}

/// `request` and the answer `read` gives it, each sent as JSON in the runner's own tests (`executor::link::sent`).
fn answered<R, T>(request: &R, read: impl FnOnce() -> Result<T, LinkError>) -> Result<T, LinkError>
where
  R: serde::Serialize + serde::de::DeserializeOwned,
  T: serde::Serialize + serde::de::DeserializeOwned,
{
  #[cfg(feature = "test-agent")]
  crate::execution::executor::link::sent::sending(request);
  #[cfg(not(feature = "test-agent"))]
  let _ = request;
  let answer = read();
  #[cfg(feature = "test-agent")]
  crate::execution::executor::link::sent::sending(&answer);
  answer
}
