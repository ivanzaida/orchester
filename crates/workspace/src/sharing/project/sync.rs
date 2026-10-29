//! Bringing a shared project's cache up to the server (`docs/architecture/server.md`, "Live
//! updates"): its project, tags, modules and tasks are read and written to the cache, with when it last
//! matched, then the moves of tasks into and out of modules the cache does not have yet. The live event stream
//! ([`crate::sharing::sync`]) keeps the cache in step otherwise; this full read runs only when the person asks
//! (*Try now*, *Check again*). A server that answers `404` for the project
//! no longer has the person as a member (10.44); the cache then says since when, and a member again is followed by its
//! stream once more. The workflow statuses this computer's runs reached go to the server then ([`super::automatic`]),
//! and so do the versions of the team's configuration published here that wait ([`RemoteResourceStore`]); the stream
//! sends those too each time it connects.

use orchester_api::ErrorCode;
use orchester_core::time::Timestamp;
use orchester_store::ProjectDb;

use super::{ProjectServer, convert, organization::tag, store::RemoteTaskStore};
use crate::{
  background,
  sharing::{ProjectAddress, SharingError, sync::RemoteResourceStore},
};

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
    let root = self.repo_root.clone();
    let after = background::run(move || ProjectDb::open(&root)?.last_module_change()).await??;
    let modules = self
      .call(|client, token, project| async move { client.modules(&token, project, after).await })
      .await?;
    let tasks = self
      .call(|client, token, project| async move { client.tasks(&token, project).await })
      .await?;
    let server = self.clone();
    // A link made before API 3 learns the project's address, its identity since, from the server; one on another
    // server is not kept, and the tasks are synced all the same. This repeats at every read, so it is not a warning.
    let address = match self.link.address {
      None => ProjectAddress::reported(&project.address, &self.entry.address)
        .inspect_err(|error| tracing::debug!(%error, "the project's address was not recorded"))
        .ok()
        .map(|address| address.to_text()),
      Some(_) => None,
    };
    background::run(move || -> Result<(), SharingError> {
      let tags = tags.iter().map(tag).collect::<Result<Vec<_>, _>>()?;
      let cached = modules
        .modules
        .iter()
        .map(convert::cached_module)
        .collect::<Result<Vec<_>, _>>()?;
      let changes: Vec<_> = modules.changes.iter().map(convert::cached_change).collect();
      let publishing = server.clone();
      let mut store = RemoteTaskStore::new(ProjectDb::open(server.repo_root())?, server)?;
      store.db.cache_shared_tags(&tags)?;
      store.db.cache_shared_modules(&cached)?;
      store.cache(&tasks.tasks, None)?;
      store.db.cache_module_changes(&changes)?;
      store
        .db
        .record_sharing_state(project.role, Some(Timestamp::now()), None)?;
      let hint = project.remote_hint.as_ref().map(|hint| hint.as_str());
      store.db.record_remote_hint(hint, Timestamp::now())?;
      if let Some(address) = address {
        store.db.record_project_address(&address)?;
      }
      super::automatic::send(&mut store)?;
      let db = ProjectDb::open(publishing.repo_root())?;
      RemoteResourceStore::new(db, publishing)?.send_waiting()
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
