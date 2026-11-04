//! The coordinator's local side of the coordinator link (`super::super::executor::link`): it answers the executor's
//! requests by reading the project database, as the executor did before it asked. Each area of the link answers in a
//! module of its own.

mod memory;
mod resources;

use std::{
  ops::Deref,
  path::{Path, PathBuf},
};

use orchester_store::{ProjectDb, ReadOnlyProjectDb, StoreError};

use crate::execution::executor::link::{CoordinatorLink, LinkError, LinkOpener};

/// The coordinator link of an executor in the coordinator's own process, over the project database `D` holds.
pub(in crate::execution) struct LocalLink<D>(D);

impl<D: Deref<Target = ProjectDb>> LocalLink<D> {
  /// The link over `db`.
  pub(in crate::execution) fn over(db: D) -> Self {
    Self(db)
  }
}

impl LocalLink<ReadOnlyProjectDb> {
  /// The link of a run's task, over a connection of its own that only reads the project database at `root`.
  pub(in crate::execution) fn open(root: &Path) -> Result<Self, StoreError> {
    ProjectDb::open_read_only(root).map(Self)
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
}

impl LinkOpener for LocalLinks {
  fn open(&self) -> Result<Box<dyn CoordinatorLink + Send>, LinkError> {
    let link = LocalLink::open(&self.root).map_err(|error| read_failed(&error))?;
    Ok(Box::new(link))
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
