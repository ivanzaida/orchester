//! Bringing a shared project's cache up to the server (`docs/architecture/server.md`, "Live
//! updates"): its project, tags and modules are read and written to the cache, then its tasks page by page, each page
//! told to the screens, then when the cache last matched and the moves of tasks into and out of modules the cache does
//! not have yet. Until the live event stream (S2d) this runs when the project is opened, every minute, after a join or
//! a link ([`super::download`], which each step reports to) and when the person asks (*Try now*, *Check again*). A
//! server that answers `404` for the project no longer has the person as a member (10.44); the cache then says since
//! when. The workflow statuses this computer's runs reached go to the server then ([`super::automatic`]).

use std::time::Instant;

use orchester_api::{ErrorCode, SharedTag, TasksPage};
use orchester_core::time::Timestamp;
use orchester_store::{CachedModule, ProjectDb};

use super::{ProjectServer, convert, organization::tag, store::RemoteTaskStore};
use crate::{
  background,
  sharing::{ProjectAddress, SharingError},
};

/// What a sync found on the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Synced {
  /// The project was read into the cache.
  Read,
  /// The server no longer has the person as a member of the project.
  Removed,
}

impl ProjectServer {
  /// Reads the server project into the cache. A server that cannot be reached leaves the cache as
  /// it is.
  pub async fn sync(&self) -> Result<(), SharingError> {
    self.sync_round(Instant::now()).await.map(drop)
  }

  /// [`Self::sync`] in the round that began at `round`, which reports to the project's first download.
  pub(super) async fn sync_round(&self, round: Instant) -> Result<Synced, SharingError> {
    super::reachable(self.servers.connection(self.link.server))?;
    let project = match self
      .call(|client, token, project| async move { client.project(&token, project).await })
      .await
    {
      Err(SharingError::Client(error)) if error.code() == Some(ErrorCode::NotFound) => {
        self.removed().await?;
        return Ok(Synced::Removed);
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
    let cached = modules
      .modules
      .iter()
      .map(convert::cached_module)
      .collect::<Result<Vec<_>, _>>()?;
    self.store_parts(&tags, cached).await?;
    self.servers.download_parts_read(&self.repo_root, round);
    self.read_tasks(round).await?;
    let server = self.clone();
    // A link made before API 3 learns the project's address, its identity since, from the server; one on another
    // server is not kept, and the tasks are synced all the same. This repeats every minute, so it is not a warning.
    let address = match self.link.address {
      None => ProjectAddress::reported(&project.address, &self.entry.address)
        .inspect_err(|error| tracing::debug!(%error, "the project's address was not recorded"))
        .ok()
        .map(|address| address.to_text()),
      Some(_) => None,
    };
    let changes: Vec<_> = modules.changes.iter().map(convert::cached_change).collect();
    background::run(move || -> Result<(), SharingError> {
      let mut store = RemoteTaskStore::new(ProjectDb::open(server.repo_root())?, server)?;
      store.db.cache_module_changes(&changes)?;
      store
        .db
        .record_sharing_state(project.role, Some(Timestamp::now()), None)?;
      let hint = project.remote_hint.as_ref().map(|hint| hint.as_str());
      store.db.record_remote_hint(hint, Timestamp::now())?;
      if let Some(address) = address {
        store.db.record_project_address(&address)?;
      }
      super::automatic::send(&mut store)
    })
    .await??;
    self.servers.shared_changed();
    Ok(Synced::Read)
  }

  /// Writes the project's tags and modules to the cache, before the tasks that name them.
  async fn store_parts(&self, tags: &[SharedTag], modules: Vec<CachedModule>) -> Result<(), SharingError> {
    let tags = tags.iter().map(tag).collect::<Result<Vec<_>, _>>()?;
    let root = self.repo_root.clone();
    background::run(move || -> Result<(), SharingError> {
      let mut db = ProjectDb::open(&root)?;
      db.cache_shared_tags(&tags)?;
      db.cache_shared_modules(&modules)?;
      Ok(())
    })
    .await??;
    Ok(())
  }

  /// Reads the project's tasks into the cache page by page, from where the first download of the round that began at
  /// `round` stopped. A page that does not move on is refused: following it would never end.
  async fn read_tasks(&self, round: Instant) -> Result<(), SharingError> {
    let mut after = self.servers.download_resume(&self.repo_root, round);
    loop {
      let page = self
        .call(|client, token, project| async move { client.tasks_page(&token, project, after).await })
        .await?;
      if page.next.is_some_and(|next| next <= after) {
        return Err(SharingError::InvalidAnswer("task page"));
      }
      let page = self.store_page(page).await?;
      self.servers.download_page_stored(&self.repo_root, &page, round);
      self.servers.shared_changed();
      match page.next {
        Some(next) => after = next,
        None => return Ok(()),
      }
    }
  }

  /// Writes `page`'s tasks to the cache.
  async fn store_page(&self, page: TasksPage) -> Result<TasksPage, SharingError> {
    let server = self.clone();
    background::run(move || -> Result<TasksPage, SharingError> {
      let mut store = RemoteTaskStore::new(ProjectDb::open(server.repo_root())?, server)?;
      store.cache(&page.tasks, None)?;
      Ok(page)
    })
    .await?
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
