//! Whose each team server in the list is (the person's decision, 2026-10-16: servers per account, as projects are):
//! the Orchester Cloud account that was signed in when this computer joined it, by the cloud's account id. Each account
//! has its own entry for a server, with its own device token in the keychain, so the same address may be listed once
//! per account. A server added while no account was signed in has no owner, and the next account to sign in takes it
//! ([`AppDb::claim_unowned_servers`]). Which account's list is shown is the workspace's decision; these are the rows it
//! reads and writes.

use orchester_core::id::{CloudAccountId, ServerId};
use rusqlite::params;

use super::{
  AppDb,
  servers::{COLUMNS, JoinedWith, ServerRow, StoredServer, update_account},
};
use crate::StoreError;

/// The servers known before this change belong to the account signed in then, if one was; the others wait for the
/// first account to sign in. An address was unique in the list; it is now unique per account, which SQLite cannot
/// change in place, so the table is made again with the same columns and checks.
pub(super) const MIGRATION: &str = r"
CREATE TABLE servers_per_account (
  id TEXT PRIMARY KEY,
  url TEXT NOT NULL,
  name TEXT NOT NULL,
  plain_http_allowed INTEGER NOT NULL CHECK (plain_http_allowed IN (0, 1)),
  added_at INTEGER NOT NULL,
  user_id TEXT,
  username TEXT,
  display_name TEXT,
  admin INTEGER CHECK (admin IN (0, 1)),
  signed_in_at INTEGER,
  certificate_pin TEXT,
  joined_with TEXT CHECK (joined_with IN ('code', 'cloud')),
  team_server TEXT CHECK ((team_server IS NOT NULL) = (joined_with IS NOT NULL AND joined_with = 'cloud')),
  owner TEXT,
  CHECK ((user_id IS NULL) = (username IS NULL)
     AND (user_id IS NULL) = (display_name IS NULL)
     AND (user_id IS NULL) = (admin IS NULL)
     AND (user_id IS NULL) = (signed_in_at IS NULL))
) STRICT;
INSERT INTO servers_per_account (id, url, name, plain_http_allowed, added_at, user_id, username, display_name, admin,
  signed_in_at, certificate_pin, joined_with, team_server, owner)
SELECT id, url, name, plain_http_allowed, added_at, user_id, username, display_name, admin, signed_in_at,
  certificate_pin, joined_with, team_server, (SELECT account_id FROM cloud_account WHERE id = 1)
FROM servers;
DROP TABLE servers;
ALTER TABLE servers_per_account RENAME TO servers;
CREATE UNIQUE INDEX servers_address_per_account ON servers (url, ifnull(owner, ''));
";

/// The condition on a `servers` row that it is the signed-in account's, read from the same database.
const SIGNED_IN_ACCOUNTS: &str = "owner IS (SELECT account_id FROM cloud_account WHERE id = 1)";

impl AppDb {
  /// The servers of `owner`, in the order they were added; `None` lists those no account took yet.
  pub fn servers_of(&self, owner: Option<&CloudAccountId>) -> Result<Vec<StoredServer>, StoreError> {
    let mut statement = self
      .connection
      .prepare(&format!("{COLUMNS} WHERE owner IS ?1 ORDER BY added_at, id"))?;
    let rows = statement
      .query_map([owner.map(ToString::to_string)], ServerRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(ServerRow::into_server).collect()
  }

  /// Every server in the list, whoever's it is, in the order they were added: for the work that keeps every server's
  /// connection and every shared project in step on this computer, never to show.
  pub fn every_server(&self) -> Result<Vec<StoredServer>, StoreError> {
    let mut statement = self.connection.prepare(&format!("{COLUMNS} ORDER BY added_at, id"))?;
    let rows = statement
      .query_map([], ServerRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(ServerRow::into_server).collect()
  }

  /// The accounts that have a server in the list, `None` for the servers no account took yet.
  pub fn server_owners(&self) -> Result<Vec<Option<CloudAccountId>>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT DISTINCT owner FROM servers ORDER BY owner")?;
    let owners = statement
      .query_map([], |row| row.get::<_, Option<String>>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    owners
      .into_iter()
      .map(|owner| {
        owner
          .map(|owner| crate::rows::parse("cloud account id", &owner))
          .transpose()
      })
      .collect()
  }

  /// Adds `server` for `owner`, with its account if it has one. An address `owner` has in the list already is refused
  /// by the table; another account's entry for it is not.
  pub fn add_server(&mut self, server: &StoredServer, owner: Option<&CloudAccountId>) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let (joined_with, team_server) = server.joined_with.map(JoinedWith::columns).unzip();
    transaction.execute(
      "INSERT INTO servers (id, url, name, plain_http_allowed, added_at, certificate_pin, joined_with, team_server, \
       owner) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
      params![
        server.id.to_string(),
        server.url,
        server.name,
        server.plain_http_allowed,
        server.added_at.unix_millis(),
        server.certificate_pin,
        joined_with,
        team_server.flatten(),
        owner.map(ToString::to_string),
      ],
    )?;
    update_account(&transaction, server.id, server.account.as_ref())?;
    transaction.commit()?;
    Ok(())
  }

  /// Gives `server` to `owner` while no account has it and `owner` has no entry at its address yet; whether it did.
  /// Which account an unowned server goes to is the workspace's decision (`sharing::cloud`): the account whose
  /// projects are linked to it, else the first to sign in.
  pub fn give_unowned_server(&mut self, server: ServerId, owner: &CloudAccountId) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "UPDATE servers SET owner = ?2 WHERE id = ?1 AND owner IS NULL AND NOT EXISTS        (SELECT 1 FROM servers AS own WHERE own.owner = ?2 AND own.url = servers.url)",
      params![server.to_string(), owner.to_string()],
    )?;
    Ok(changed == 1)
  }

  /// Removes `server` while no account has it; whether it did. Its device token in the keychain is the caller's to
  /// remove.
  pub fn forget_unowned_server(&mut self, server: ServerId) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      "DELETE FROM servers WHERE id = ?1 AND owner IS NULL",
      [server.to_string()],
    )?;
    Ok(changed == 1)
  }

  /// Whether `server` is the signed-in Orchester Cloud account's (nobody's while none is signed in).
  pub fn is_signed_in_accounts_server(&self, server: ServerId) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      &format!("SELECT EXISTS (SELECT 1 FROM servers WHERE id = ?1 AND {SIGNED_IN_ACCOUNTS})"),
      [server.to_string()],
      |row| row.get(0),
    )?)
  }

  /// Forgets who is signed in to `server` from this computer, in one statement with the check that `server` is the
  /// signed-in Orchester Cloud account's (nobody's while none is signed in); whether it was, and so whether anything
  /// changed.
  pub fn sign_out_of_server(&mut self, server: ServerId) -> Result<bool, StoreError> {
    let changed = self.connection.execute(
      &format!(
        "UPDATE servers SET user_id = NULL, username = NULL, display_name = NULL, admin = NULL, signed_in_at = NULL          WHERE id = ?1 AND {SIGNED_IN_ACCOUNTS}"
      ),
      [server.to_string()],
    )?;
    Ok(changed == 1)
  }
}
