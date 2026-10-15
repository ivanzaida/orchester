//! Bringing a shared project's cache up to the server (`docs/architecture/server.md`, "Live
//! updates"): its project, tags and tasks are read and written to the cache, with when it last
//! matched. Until the live event stream (S2d) this runs when the project is opened, every minute
//! and when the person asks (*Try now*, *Check again*). A server that answers `404` for the project
//! no longer has the person as a member (10.44); the cache then says since when. The workflow
//! statuses this computer's runs reached go to the server then ([`super::automatic`]).

use orchester_api::ErrorCode;
use orchester_core::time::Timestamp;
use orchester_store::ProjectDb;

use super::{ProjectServer, organization::tag, store::RemoteTaskStore};
use crate::{background, sharing::SharingError};

impl ProjectServer {
  /// Reads the server project into the cache. A server that cannot be reached leaves the cache as
  /// it is.
  pub async fn sync(&self) -> Result<(), SharingError> {
    super::reachable(self.servers.connection(self.link.server))?;
    let project = match self
      .call(|client, token, project| async move { client.project(&token, project).await })
      .await
    {
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::NotFound) => {
        return self.removed().await;
      }
      answer => answer?,
    };
    let tags = self
      .call(|client, token, project| async move { client.tags(&token, project).await })
      .await?;
    let tasks = self
      .call(|client, token, project| async move { client.tasks(&token, project).await })
      .await?;
    let server = self.clone();
    // A link made before API 3 learns the project's address, its identity since, from the server.
    let address = self.link.address.is_none().then_some(project.address);
    background::run(move || -> Result<(), SharingError> {
      let tags = tags.iter().map(tag).collect::<Result<Vec<_>, _>>()?;
      let mut store = RemoteTaskStore::new(ProjectDb::open(server.repo_root())?, server)?;
      store.db.cache_shared_tags(&tags)?;
      store.cache(&tasks.tasks, None)?;
      store
        .db
        .record_sharing_state(project.role, Some(Timestamp::now()), None)?;
      if let Some(address) = address {
        store.db.record_project_address(&address)?;
      }
      super::automatic::send(&mut store)
    })
    .await??;
    self.servers.shared_changed();
    Ok(())
  }

  /// The server no longer has the person as a member of the project.
  async fn removed(&self) -> Result<(), SharingError> {
    if self.link.removed_at.is_some() {
      return Ok(());
    }
    let (root, role) = (self.repo_root.clone(), self.link.role);
    background::run(move || ProjectDb::open(&root)?.record_sharing_state(role, None, Some(Timestamp::now()))).await??;
    self.servers.shared_changed();
    Ok(())
  }
}
