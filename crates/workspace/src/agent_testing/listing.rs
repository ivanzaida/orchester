//! What Project settings › Services lists (`01 · Services` `G1sdS`): the project's services, stacks and test suites,
//! each as its draft if it has one, otherwise its latest version. Blocks on SQLite.

use std::path::Path;

use orchester_core::{
  resource::TestSuite,
  service::{Service, Stack},
};
use orchester_store::{ProjectDb, StoreError};

use super::data::{Entry, entries};

/// The project's services, stacks and test suites, each ordered by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectServices {
  pub services: Vec<Entry<Service>>,
  pub stacks: Vec<Entry<Stack>>,
  pub suites: Vec<Entry<TestSuite>>,
}

impl ProjectServices {
  /// What the project at `root` defines.
  pub fn load_in(root: &Path) -> Result<Self, StoreError> {
    let db = ProjectDb::open(root)?;
    Ok(Self {
      services: entries(&db)?,
      stacks: entries(&db)?,
      suites: entries(&db)?,
    })
  }
}
