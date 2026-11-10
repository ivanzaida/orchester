//! Which pipeline New task offers first: the project's default, if a person chose one.

use std::path::Path;

use orchester_core::id::ResourceId;
use orchester_store::{ProjectDb, StoreError};

/// The default pipeline of the project at `repo_root`; `None` while a person has not chosen one. It may be archived:
/// New task then offers none of it, and the hub does not call it the default.
pub fn default_in(repo_root: &Path) -> Result<Option<ResourceId>, StoreError> {
  ProjectDb::open(repo_root)?.default_pipeline()
}
