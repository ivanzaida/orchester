//! How the Orchestrator watches a project's runs (Project settings › Orchestrator › *Watching*): the default order
//! and when a run without progress is reported, kept as set without versions ([`save_watch_in`]). Saving counts the
//! project's change, so what shows them reads them again.

use std::path::Path;

use orchester_core::orchestrator::WatchSettings;
use orchester_store::ProjectDb;

use super::Workspace;
use crate::{
  messages::{self, Message},
  orchestrator::settings::save_watch_in,
  tasks::outside::Outside,
};

impl Workspace {
  /// Keeps `watch` for the project at `repo_root` and counts the change once it is saved. Blocks on SQLite.
  pub fn set_orchestrator_watch(&self, repo_root: &Path, watch: &WatchSettings) -> Result<(), Message> {
    let project = ProjectDb::open(repo_root)
      .map_err(|error| messages::store_error(&error))?
      .project()
      .id();
    save_watch_in(repo_root, watch)?;
    self.changes.record(project, Outside::Orchestrator);
    Ok(())
  }
}
