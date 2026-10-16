//! Servers per Orchester Cloud account (the person's decision, 2026-10-16): the person and automation reach only the
//! signed-in account's servers, and another account's server is not found ([`SharingError::ServerMissing`]), whatever
//! asks for it: a screen, a domain tool or the Orchestrator chat. The background work that keeps every server's
//! connection and every shared project in step goes on for each account's servers while another is signed in, and
//! shows the signed-in account nothing of it: each shared project syncs through its own account's server only.

use orchester_core::id::{CloudAccountId, ServerId};

use super::{ServerEntry, Servers, SharingError};
use crate::background;

/// On whose behalf a request reaches a server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Behalf {
  /// The signed-in account, for the person or automation: only its own servers.
  Account,
  /// The background work that keeps each account's servers and shared projects in step, and shows nothing.
  Background,
}

impl Servers {
  /// Every server in the list, whichever account's, with its connection: for the background work only. Blocking.
  pub(crate) fn every_entry(&self) -> Result<Vec<ServerEntry>, SharingError> {
    self.listed(self.inner.workspace.every_server()?)
  }

  /// `owner`'s servers with their connections: for the background work on `owner`'s shared projects. Blocking.
  pub(crate) fn entries_of(&self, owner: Option<&CloudAccountId>) -> Result<Vec<ServerEntry>, SharingError> {
    self.listed(self.inner.workspace.servers_of(owner)?)
  }

  /// Refuses `server` unless it is one of the signed-in account's: another account's server is not found.
  pub(super) async fn own(&self, server: ServerId) -> Result<(), SharingError> {
    let workspace = self.inner.workspace.clone();
    if background::run(move || workspace.is_own_server(server)).await?? {
      Ok(())
    } else {
      Err(SharingError::ServerMissing)
    }
  }
}
