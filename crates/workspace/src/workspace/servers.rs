//! The team servers in the list belong to the Orchester Cloud account that was signed in when this computer joined them
//! (the person's decision, 2026-10-16: servers per account, as projects are). The list and every use of a server for
//! the person or automation are the signed-in account's alone ([`Workspace::servers`]); another account's servers are
//! not found. Only the work that keeps every server's connection and every shared project in step on this computer, and
//! shows none of it, reads them all. Only this crate changes a server's row, after it checked whose the server is.

use orchester_core::id::{CloudAccountId, ServerId};
use orchester_store::{RegisteredProject, ServerAccount, StoreError, StoredServer};

use super::{Workspace, owner};

impl Workspace {
  /// The signed-in account's team servers, in the order they were added; while none is signed in, those no account
  /// took yet.
  pub fn servers(&self) -> Result<Vec<StoredServer>, StoreError> {
    let db = self.db.lock();
    db.servers_of(owner(&db)?.as_ref())
  }

  /// The Orchester Cloud account signed in now, whose servers and projects are shown; `None` while none is.
  pub(crate) fn signed_in_account(&self) -> Result<Option<CloudAccountId>, StoreError> {
    owner(&self.db.lock())
  }

  /// `owner`'s team servers, in the order they were added: for a join that began while `owner` was signed in, and for
  /// the background work on `owner`'s shared projects.
  pub(crate) fn servers_of(&self, owner: Option<&CloudAccountId>) -> Result<Vec<StoredServer>, StoreError> {
    self.db.lock().servers_of(owner)
  }

  /// `owner`'s projects, for the background work on its shared projects, never to show.
  pub(crate) fn projects_of(&self, owner: Option<&CloudAccountId>) -> Result<Vec<RegisteredProject>, StoreError> {
    self.db.lock().projects_of(owner)
  }

  /// Every team server in the list, whichever account's: for the work that keeps each one's connection current, never
  /// to show or to act on for the person.
  pub(crate) fn every_server(&self) -> Result<Vec<StoredServer>, StoreError> {
    self.db.lock().every_server()
  }

  /// The accounts with a server in the list (`None` for those no account took yet), for the background work on each
  /// one's shared projects.
  pub(crate) fn server_owners(&self) -> Result<Vec<Option<CloudAccountId>>, StoreError> {
    self.db.lock().server_owners()
  }

  /// Adds `server` as `owner`'s.
  pub(crate) fn add_server(&self, server: &StoredServer, owner: Option<&CloudAccountId>) -> Result<(), StoreError> {
    self.db.lock().add_server(server, owner)
  }

  /// Whether `server` is the signed-in account's, read in one statement.
  pub(crate) fn is_own_server(&self, server: ServerId) -> Result<bool, StoreError> {
    self.db.lock().is_signed_in_accounts_server(server)
  }

  /// Forgets who is signed in to `server` from this computer if it is the signed-in account's, checked in the same
  /// statement; whether it was.
  pub(crate) fn sign_out_of_server(&self, server: ServerId) -> Result<bool, StoreError> {
    self.db.lock().sign_out_of_server(server)
  }

  /// Records how this computer trusts `server`: its pinned certificate, or the system's trust store and whether plain
  /// HTTP is allowed there.
  pub(crate) fn set_server_trust(
    &self,
    server: ServerId,
    pin: Option<&str>,
    plain_http: bool,
  ) -> Result<(), StoreError> {
    self.db.lock().set_server_trust(server, pin, plain_http)
  }

  /// Records the name the server gives itself now.
  pub(crate) fn set_server_name(&self, server: ServerId, name: &str) -> Result<(), StoreError> {
    self.db.lock().set_server_name(server, name)
  }

  /// Records who is signed in to `server` from this computer; `None` after signing out.
  pub(crate) fn set_server_account(&self, server: ServerId, account: Option<&ServerAccount>) -> Result<(), StoreError> {
    self.db.lock().set_server_account(server, account)
  }
}
