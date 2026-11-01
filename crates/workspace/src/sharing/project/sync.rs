//! Bringing a shared project's cache up to the server (`docs/architecture/server.md`, "Live
//! updates"): its project, tags, modules and tasks are read and written to the cache, with when it last
//! matched, then the moves of tasks into and out of modules the cache does not have yet. The live event stream
//! ([`crate::sharing::sync`]) keeps the cache in step otherwise; this full read runs only when the person asks
//! (*Try now*, *Check again*). A server that answers `404` for the project
//! no longer has the person as a member (10.44); the cache then says since when, and a member again is followed by its
//! stream once more. The workflow statuses this computer's runs reached go to the server then ([`super::automatic`]),
//! and so do the versions of the team's configuration published here that wait ([`RemoteResourceStore`]); the stream
//! sends those too each time it connects.

use std::time::Instant;

use orchester_api::{ErrorCode, SharedTag, TasksPage};
use orchester_core::time::Timestamp;
use orchester_store::{CachedModule, ProjectDb};

use super::{ProjectServer, convert, organization::tag, store::RemoteTaskStore};
use crate::{
  background,
  sharing::{ProjectAddress, SharingError, sync::RemoteResourceStore},
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
    let hint = project.remote_hint.as_ref().map(|hint| hint.as_str().to_owned());
    self.store_parts(&tags, cached, hint).await?;
    self.servers.download_parts_read(&self.repo_root, round);
    self.read_tasks(round).await?;
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
    let changes: Vec<_> = modules.changes.iter().map(convert::cached_change).collect();
    background::run(move || -> Result<(), SharingError> {
      let publishing = server.clone();
      let mut store = RemoteTaskStore::new(ProjectDb::open(server.repo_root())?, server)?;
      store.db.cache_module_changes(&changes)?;
      store
        .db
        .record_sharing_state(project.role, Some(Timestamp::now()), None)?;
      if let Some(address) = address {
        store.db.record_project_address(&address)?;
      }
      super::automatic::send(&mut store)?;
      let db = ProjectDb::open(publishing.repo_root())?;
      RemoteResourceStore::new(db, publishing)?.send_waiting()
    })
    .await??;
    self.servers.shared_changed();
    Ok(Synced::Read)
  }

  /// Writes the project's tags and modules to the cache, before the tasks that name them, and its repository `hint`,
  /// which the first download's page compares the checkout's `origin` with while the tasks still come down.
  async fn store_parts(
    &self,
    tags: &[SharedTag],
    modules: Vec<CachedModule>,
    hint: Option<String>,
  ) -> Result<(), SharingError> {
    let tags = tags.iter().map(tag).collect::<Result<Vec<_>, _>>()?;
    let root = self.repo_root.clone();
    background::run(move || -> Result<(), SharingError> {
      let mut db = ProjectDb::open(&root)?;
      db.cache_shared_tags(&tags)?;
      db.cache_shared_modules(&modules)?;
      db.record_remote_hint(hint.as_deref(), Timestamp::now())?;
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
      if !moves_on(&page, after) {
        return Err(SharingError::InvalidAnswer("task page"));
      }
      let page = self.store_page(page).await?;
      self.servers.download_page_stored(&self.repo_root, &page, round);
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

/// Whether `page`, read after `after`, ends or leads somewhere: one that names a next page starts later and brings
/// tasks above `after`, or a server could keep a round going almost without end, with every other project waiting
/// behind it.
fn moves_on(page: &TasksPage, after: u32) -> bool {
  page
    .next
    .is_none_or(|next| next > after && page.tasks.iter().any(|task| task.number > after))
}
