//! Project settings › Environment as data: the variables the project adds to the environment of
//! every agent CLI and command tool its runs start. Blocks on SQLite; the UI calls it off the UI
//! thread.

use std::path::Path;

use orchester_core::{resource::CommandEnvironment, store::SettingsStore};
use orchester_store::{ProjectDb, StoreError};

pub fn load_in(repo_root: &Path) -> Result<CommandEnvironment, StoreError> {
  ProjectDb::open(repo_root)?.project_environment()
}

pub fn save_in(repo_root: &Path, variables: &CommandEnvironment) -> Result<(), StoreError> {
  ProjectDb::open(repo_root)?.save_project_environment(variables)
}
