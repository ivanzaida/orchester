//! What uses a resource, read for the project at a path in the session its team server has here: the window's
//! confirmation asks it before the person archives.

use std::path::Path;

use orchester_core::id::ResourceId;
use orchester_store::ProjectDb;

use super::{UsedBy, UsedByError};
use crate::{background, sharing::Servers};

impl Servers {
  /// What uses `resource` in the project at `root`: asked of its team server in this computer's session of it, or
  /// computed from the database for a local project ([`super::used_by`]). Nothing is archived or changed.
  pub async fn used_by(&self, root: &Path, resource: ResourceId) -> Result<UsedBy, UsedByError> {
    let owned = root.to_owned();
    let link = background::run(move || ProjectDb::open(&owned)?.sharing())
      .await
      .map_err(|_| UsedByError::Interrupted)??;
    let session = match link {
      Some(link) => Some(
        self
          .team_session(link.server, link.project)
          .await
          .map_err(UsedByError::Session)?,
      ),
      None => None,
    };
    super::used_by(root, session.as_ref(), resource).await
  }
}
