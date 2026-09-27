//! Another project's database, opened to read it (Import resources): it is never written.

use std::{ops::Deref, path::Path};

use super::{DIRECTORY, ProjectDb, database_path, load_project, schema};
use crate::{StoreError, migrate};

/// A project database opened only to read it. SQLite refuses every write through its connection,
/// and only `&ProjectDb` is handed out, so no method that writes can be called on it.
pub struct ReadOnlyProjectDb(ProjectDb);

impl Deref for ReadOnlyProjectDb {
  type Target = ProjectDb;

  fn deref(&self) -> &ProjectDb {
    &self.0
  }
}

impl ProjectDb {
  /// Opens the initialized repository `repo_root` to read it. Nothing is migrated: a database
  /// this build has not migrated yet is refused ([`StoreError::SchemaTooOld`]) until the project
  /// is opened.
  pub fn open_read_only(repo_root: &Path) -> Result<ReadOnlyProjectDb, StoreError> {
    let database = database_path(repo_root);
    if !database.is_file() {
      return Err(StoreError::NotInitialized(repo_root.to_path_buf()));
    }
    let connection = migrate::open_read_only(&database, schema::MIGRATIONS)?;
    let project = load_project(&connection)?;
    Ok(ReadOnlyProjectDb(Self {
      connection,
      project,
      directory: repo_root.join(DIRECTORY),
    }))
  }
}
