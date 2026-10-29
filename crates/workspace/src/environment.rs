//! Project settings › Environment as data: the variables the project adds to the environment of
//! every agent CLI and command tool its runs start. Blocks on SQLite; the UI calls it off the UI
//! thread.

use std::path::Path;

use orchester_core::{permission::widening::changed_variables, resource::CommandEnvironment, store::SettingsStore};
use orchester_store::{ProjectDb, StoreError};

use crate::messages::{self, Message};

pub fn load_in(repo_root: &Path) -> Result<CommandEnvironment, StoreError> {
  ProjectDb::open(repo_root)?.project_environment()
}

pub fn save_in(repo_root: &Path, variables: &CommandEnvironment) -> Result<(), StoreError> {
  ProjectDb::open(repo_root)?.save_project_environment(variables)
}

/// Any changed variable except the harmless ones, including removals. Shared by UI and MCP.
pub fn widenings(old: &CommandEnvironment, new: &CommandEnvironment) -> Vec<Message> {
  changed_variables(old, new)
    .into_iter()
    .map(crate::tools::widening::variable)
    .collect()
}

/// Saves only the exact change that was inspected (and, for a widening, confirmed by the person).
pub fn save_checked_in(
  root: &Path,
  base: &CommandEnvironment,
  asked: &CommandEnvironment,
  confirmed: bool,
) -> Result<(), Message> {
  if !confirmed && !widenings(base, asked).is_empty() {
    return Err(Message::new("projectSettings.personOnly"));
  }
  let saved = ProjectDb::open(root)
    .and_then(|mut db| db.save_environment_if_current(base, asked))
    .map_err(|error| messages::store_error(&error))?;
  if !saved {
    return Err(Message::new("projectSettings.changedOutside"));
  }
  Ok(())
}
