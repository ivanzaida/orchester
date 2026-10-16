//! Whose the team servers nobody owns become when an account signs in (servers per account, 2026-10-16). They are
//! servers known before servers had owners while no account was signed in, or joined while none was. A device token is
//! the account's that joined with it, which the list no longer knows; the projects linked to the server say it:
//!
//! - A server that projects link to goes to the account that owns those projects, whoever signs in: another account
//!   never gets its token, and those projects keep working when their account signs in. One that projects of several
//!   accounts link to, or whose projects' account has an entry at its address already, stays nobody's and hidden: it is
//!   never given to an account that did not join it, and it is kept for the projects linked to it.
//! - A server no project links to goes to the account signing in, unless that account has an entry at its address
//!   already. Such a duplicate is removed with its device token: kept, it would go to the next account to sign in, and
//!   nothing on this computer uses it. The server ends that unused session by itself after 90 days.

use std::collections::{BTreeSet, HashMap};

use orchester_core::{
  id::{CloudAccountId, ServerId},
  secret::{VaultError, VaultKey},
};
use orchester_store::{AppDb, ProjectDb, StoreError};

use crate::{secrets::Vault, workspace::Workspace};

/// The accounts whose projects link to each server, read before a sign-in writes; `None`: projects no account took
/// yet, which the account signing in takes in the same write.
pub(super) struct Links(HashMap<ServerId, BTreeSet<Option<CloudAccountId>>>);

impl Links {
  /// Reads the link of every project on this computer whose folder is there. A project whose link cannot be read
  /// counts for no server. Blocking.
  pub(super) fn read(workspace: &Workspace) -> Result<Self, StoreError> {
    let projects = workspace.db.lock().owned_projects()?;
    let mut links: HashMap<ServerId, BTreeSet<Option<CloudAccountId>>> = HashMap::new();
    for (owner, project) in projects.into_iter().filter(|(_, project)| project.repo_root.is_dir()) {
      match ProjectDb::open(&project.repo_root).and_then(|db| db.sharing()) {
        Ok(Some(link)) => {
          links.entry(link.server).or_default().insert(owner);
        }
        Ok(None) => {}
        // By id: the project may be another account's, whose folders the signed-in account's logs do not name.
        Err(error) => tracing::debug!(%error, project = %project.id, "cannot read whether a project is shared"),
      }
    }
    Ok(Self(links))
  }

  /// The accounts whose projects link to `server`, with those no account took yet counted as `account`'s.
  fn owners(&self, server: ServerId, account: &CloudAccountId) -> BTreeSet<CloudAccountId> {
    self.0.get(&server).map_or_else(BTreeSet::new, |owners| {
      owners.iter().map(|owner| owner.unwrap_or(*account)).collect()
    })
  }
}

/// Gives each server nobody owns to its account as `links` say, in the sign-in's write for `account`; the duplicates
/// it removed, whose device tokens [`forget_tokens`] removes once the database is let go.
pub(super) fn claim_servers(
  db: &mut AppDb,
  account: &CloudAccountId,
  links: &Links,
) -> Result<Vec<ServerId>, StoreError> {
  let mut removed = Vec::new();
  for server in db.servers_of(None)? {
    let owners = links.owners(server.id, account);
    let mut owners = owners.iter();
    match (owners.next(), owners.next()) {
      (None, _) => {
        if !db.give_unowned_server(server.id, account)? && db.forget_unowned_server(server.id)? {
          removed.push(server.id);
        }
      }
      (Some(owner), None) => {
        if !db.give_unowned_server(server.id, owner)? {
          tracing::warn!(server = %server.id, "a server nobody owns stays hidden: its projects' account has its address");
        }
      }
      (Some(_), Some(_)) => {
        tracing::warn!(server = %server.id, "a server nobody owns stays hidden: projects of several accounts link to it");
      }
    }
  }
  Ok(removed)
}

/// Removes the device tokens of the servers [`claim_servers`] removed. Blocking.
pub(super) fn forget_tokens(vault: &Vault, removed: &[ServerId]) {
  for server in removed {
    match vault.get().delete(&VaultKey::server_token(*server)) {
      Ok(()) | Err(VaultError::NotFound) => {}
      // The row is gone: nothing reads the token again, and the server ends its session once unused.
      Err(error) => tracing::warn!(%error, %server, "the device token of a removed server stays in the keychain"),
    }
  }
}
