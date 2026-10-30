//! Whose each team server in the list is (the person's decisions, 2026-10-16 and 2026-10-30: servers per account, as
//! projects are): the Orchester Cloud account that was signed in when this computer joined it, by the cloud's account
//! id, or nobody's: this computer's, like a project added with no account signed in. Each owner has its own entry for a
//! server, with its own device token in the keychain, so the same address may be listed once per owner. No sign-in
//! claims a server of this computer's: it shows beside the account's own. Which list is shown is the workspace's
//! decision; these are the rows it reads and writes.

use orchester_core::id::{CloudAccountId, ServerId};
use rusqlite::{OptionalExtension, params};

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

/// The condition on a `servers` row that the signed-in account uses it, read from the same database: this computer's,
/// or the account's own.
const SIGNED_IN_ACCOUNTS: &str = "(owner IS NULL OR owner = (SELECT account_id FROM cloud_account WHERE id = 1))";

impl AppDb {
  /// The servers of `owner`, in the order they were added; `None` lists this computer's.
  pub fn servers_of(&self, owner: Option<&CloudAccountId>) -> Result<Vec<StoredServer>, StoreError> {
    let mut statement = self
      .connection
      .prepare(&format!("{COLUMNS} WHERE owner IS ?1 ORDER BY added_at, id"))?;
    let rows = statement
      .query_map([owner.map(ToString::to_string)], ServerRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(ServerRow::into_server).collect()
  }

  /// The servers shown while `owner` is signed in (`None`: nobody), in the order they were added: this computer's and
  /// `owner`'s own.
  pub fn servers_visible_to(&self, owner: Option<&CloudAccountId>) -> Result<Vec<StoredServer>, StoreError> {
    let mut statement = self.connection.prepare(&format!(
      "{COLUMNS} WHERE owner IS NULL OR owner = ?1 ORDER BY added_at, id"
    ))?;
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

  /// Whose `server` is, `None` when it is not in the list.
  pub fn server_owner(&self, server: ServerId) -> Result<Option<Option<CloudAccountId>>, StoreError> {
    let owner: Option<Option<String>> = self
      .connection
      .query_row("SELECT owner FROM servers WHERE id = ?1", [server.to_string()], |row| {
        row.get(0)
      })
      .optional()?;
    owner
      .map(|owner| {
        owner
          .map(|owner| crate::rows::parse("cloud account id", &owner))
          .transpose()
      })
      .transpose()
  }

  /// Whether `server` is one the signed-in Orchester Cloud account uses: this computer's, or the account's own.
  pub fn is_signed_in_accounts_server(&self, server: ServerId) -> Result<bool, StoreError> {
    Ok(self.connection.query_row(
      &format!("SELECT EXISTS (SELECT 1 FROM servers WHERE id = ?1 AND {SIGNED_IN_ACCOUNTS})"),
      [server.to_string()],
      |row| row.get(0),
    )?)
  }

  /// Forgets who is signed in to `server` from this computer, in one statement with the check that it is one the
  /// signed-in Orchester Cloud account uses; whether it was, and so whether anything changed.
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
