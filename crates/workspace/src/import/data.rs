//! Reading the project to import from and writing the copies into this one. The other project's
//! database is only read ([`ProjectDb::open_read_only`]). Every function blocks on SQLite; the UI
//! calls them off the UI thread.

use std::path::Path;

use orchester_core::{
  id::ProjectId,
  import::{Catalog, ImportBatch},
  resource::Resource,
  store::ImportStore,
};
use orchester_store::{ProjectDb, RegisteredProject, StoreError};
use thiserror::Error;

use crate::{
  messages::{self, Message},
  workspace::Workspace,
};

/// The projects resources can be imported from: every project of the workspace but `current`,
/// most recently opened first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sources(pub Vec<RegisteredProject>);

impl lurq::app::component::DevtoolsInspectable for Sources {}

pub fn sources(workspace: &Workspace, current: ProjectId) -> Result<Sources, StoreError> {
  let mut projects = workspace.projects()?;
  projects.retain(|project| project.id != current);
  Ok(Sources(projects))
}

/// What the project to import from and this project hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalogs {
  pub source: Catalog,
  pub target: Catalog,
}

impl lurq::app::component::DevtoolsInspectable for Catalogs {}

#[derive(Debug, Error)]
pub enum LoadError {
  #[error("the project to import from: {0}")]
  Source(#[source] StoreError),
  #[error("this project: {0}")]
  Target(#[source] StoreError),
}

/// Reads the project at `source` without changing it, then this project at `target`.
pub fn load(source: &Path, target: &Path) -> Result<Catalogs, LoadError> {
  let source = {
    let db = ProjectDb::open_read_only(source).map_err(LoadError::Source)?;
    Catalog::read(db.project().clone(), &*db).map_err(LoadError::Source)?
  };
  let db = ProjectDb::open(target).map_err(LoadError::Target)?;
  let target = Catalog::read(db.project().clone(), &db).map_err(LoadError::Target)?;
  Ok(Catalogs { source, target })
}

/// Writes `batch` into the project at `target`: every copy as a draft, at once.
pub fn import_into(target: &Path, batch: &ImportBatch) -> Result<Vec<Resource>, StoreError> {
  ProjectDb::open(target)?.import(batch)
}

/// Why the project called `source` could not be read, or this one.
pub fn load_error(error: &LoadError, source: &str) -> Message {
  let named = |key| Message::new(key).with("project", source.to_owned());
  match error {
    LoadError::Source(StoreError::NotInitialized(_)) => named("projectImport.errors.sourceMissing"),
    LoadError::Source(StoreError::SchemaTooOld { .. }) => named("projectImport.errors.sourceOutdated"),
    LoadError::Source(StoreError::SchemaTooNew { .. }) => named("projectImport.errors.sourceNewer"),
    LoadError::Source(_) => named("projectImport.errors.sourceUnreadable"),
    LoadError::Target(error) => messages::store_error(error),
  }
}

/// Why the copies could not be written. Nothing was: the import is one transaction.
pub fn import_error(error: &StoreError) -> Message {
  match error {
    StoreError::ResourceNotFound(_)
    | StoreError::ConnectionNotFound(_)
    | StoreError::SecretNotFound(_)
    | StoreError::DuplicateSlug { .. } => Message::new("projectImport.errors.targetChanged"),
    other => messages::store_error(other),
  }
}
