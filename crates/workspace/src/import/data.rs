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
  match error {
    LoadError::Source(error) => source_error(error, source),
    LoadError::Target(error) => messages::store_error(error),
  }
}

fn source_error(error: &StoreError, source: &str) -> Message {
  let named = |key| Message::new(key).with("project", source.to_owned());
  match error {
    StoreError::NotInitialized(_) => named("projectImport.errors.sourceMissing"),
    StoreError::SchemaTooOld { .. } => named("projectImport.errors.sourceOutdated"),
    StoreError::SchemaTooNew { .. } => named("projectImport.errors.sourceNewer"),
    _ => named("projectImport.errors.sourceUnreadable"),
  }
}

/// What a page says when the project called `source`, or this one, could not be read: what happened, and what
/// can be done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadNotice {
  pub title: Message,
  pub body: Message,
  /// The file system refused reading the folder, rather than its data being unreadable.
  pub refused: bool,
}

pub fn load_notice(error: &LoadError, source: &str) -> LoadNotice {
  match error {
    LoadError::Source(error) => match refused_folder(error) {
      Some(folder) => LoadNotice {
        title: Message::new("projectImport.errors.sourceRefused").with("project", source.to_owned()),
        body: Message::new("projectImport.errors.sourceRefusedDetail").with("folder", folder),
        refused: true,
      },
      None => LoadNotice {
        title: source_error(error, source),
        body: Message::new("projectImport.errors.chooseAnother").with("project", source.to_owned()),
        refused: false,
      },
    },
    LoadError::Target(error) => LoadNotice {
      title: Message::new("projectImport.errors.targetUnreadable"),
      body: messages::store_error(error),
      refused: refused_folder(error).is_some(),
    },
  }
}

/// The folder whose access the file system refused, if that is why `error` happened.
fn refused_folder(error: &StoreError) -> Option<String> {
  match error {
    StoreError::Io { path, source } if source.kind() == std::io::ErrorKind::PermissionDenied => {
      Some(path.display().to_string())
    }
    _ => None,
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

/// Why writing the copies failed, as the page tells it apart. Nothing was written, and the choices are kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportFailure {
  /// This project changed while the person chose: something the plan uses is gone, or a copy's slug was taken.
  Changed,
  /// The file system refused writing to this project's folder.
  Refused { folder: String },
  /// Anything else, with why; the log has the details.
  Failed(Message),
}

pub fn import_failure(error: &StoreError) -> ImportFailure {
  if let Some(folder) = refused_folder(error) {
    return ImportFailure::Refused { folder };
  }
  match error {
    StoreError::ResourceNotFound(_)
    | StoreError::ConnectionNotFound(_)
    | StoreError::SecretNotFound(_)
    | StoreError::DuplicateSlug { .. } => ImportFailure::Changed,
    other => ImportFailure::Failed(messages::store_error(other)),
  }
}
