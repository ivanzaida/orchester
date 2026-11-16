//! What Project settings › Services lists (`01 · Services` `G1sdS`): the project's services, stacks and test suites,
//! each as its draft if it has one, otherwise its latest version; and the published services as a run starts them,
//! which a task's Services tab describes its services by. Blocks on SQLite.

use std::path::Path;

use orchester_core::{
  resource::{ResourceBody, TestSuite},
  service::{Service, Stack},
  store::ResourceStore,
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

/// The latest published version of each of the project's services at `root`, removed ones left out: what a run
/// starts.
pub fn published_services_in(root: &Path) -> Result<Vec<Service>, StoreError> {
  let db = ProjectDb::open_read_only(root)?;
  let mut services = Vec::new();
  for resource in db.resources(Service::KIND)? {
    services.extend(
      db.latest_version::<Service>(resource.id())?
        .map(|version| version.body().clone()),
    );
  }
  Ok(services)
}
