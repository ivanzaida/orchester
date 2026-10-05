//! A project feed's sharing: whether the project's task list is shared, with its server's
//! connection, read again whenever a shared project's cache changed. Opening a shared project asks
//! for its cache to be read from the server at once.

use std::path::{Path, PathBuf};

use lurq::{
  app::ctx::{Ctx, StreamEmitter},
  core::Signal,
};
use orchester_store::{ProjectDb, SharingLink};

use crate::{
  background, messages,
  sharing::{Servers, feed::ServerFeed, project::state::ProjectSharing},
};

/// The link as the future holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Link(Option<SharingLink>);

impl lurq::app::component::DevtoolsInspectable for Link {}

/// How often shared projects' caches changed, for reading what depends on them again.
pub fn changes(ctx: &mut Ctx, servers: &Servers) -> u64 {
  let count = ctx.stream(servers.clone(), |servers, emitter: StreamEmitter<u64, ()>| async move {
    let mut updates = servers.subscribe_shared();
    loop {
      let revision = *updates.borrow_and_update();
      if !emitter.emit(revision) || updates.changed().await.is_err() {
        return;
      }
    }
  });
  count.state().get().data.unwrap_or(0)
}

/// The sharing of the project at `root` as of `changes`; `None` while it is local or not read yet.
/// `synced` records that this feed asked for the first sync.
pub fn sharing(ctx: &mut Ctx, (root, changes): (&Path, u64), synced: &Signal<bool>) -> Option<ProjectSharing> {
  let feed = ServerFeed::of(ctx);
  let load = ctx.future((root.to_path_buf(), changes), |(root, _): (PathBuf, u64)| async move {
    background::run(move || ProjectDb::open(&root).and_then(|db| db.sharing()))
      .await?
      .map(Link)
      .map_err(|error| {
        tracing::error!(%error, "cannot read the project's sharing");
        messages::store_error(&error)
      })
  });
  let link = load.state().get().data.and_then(|Link(link)| link)?;
  if !synced.get_untracked() {
    synced.set(true);
    feed.servers.sync_now();
  }
  let list = feed.list.get();
  Some(ProjectSharing::new(link.clone(), list.entry(link.server)))
}
