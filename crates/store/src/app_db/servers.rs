//! The shared servers the person added (`docs/architecture/server.md`, "Desktop data"): each one's
//! address, name, whether plain HTTP is allowed for it, and the account signed in there. The token
//! of that account is in the OS keychain, never here.

use orchester_core::{
  id::{ServerId, UserId},
  time::Timestamp,
};
use rusqlite::{Row, params};

use super::AppDb;
use crate::{StoreError, rows::parse};

pub(super) const MIGRATION: &str = r"
CREATE TABLE servers (
  id TEXT PRIMARY KEY,
  url TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  plain_http_allowed INTEGER NOT NULL CHECK (plain_http_allowed IN (0, 1)),
  added_at INTEGER NOT NULL,
  user_id TEXT,
  username TEXT,
  display_name TEXT,
  admin INTEGER CHECK (admin IN (0, 1)),
  signed_in_at INTEGER,
  CHECK ((user_id IS NULL) = (username IS NULL)
     AND (user_id IS NULL) = (display_name IS NULL)
     AND (user_id IS NULL) = (admin IS NULL)
     AND (user_id IS NULL) = (signed_in_at IS NULL))
) STRICT;
";

/// A server in the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredServer {
  pub id: ServerId,
  /// The normalized address the desktop checked when the server was added.
  pub url: String,
  /// What the server calls itself (`GET /server`).
  pub name: String,
  /// The person's opt-in to plain HTTP for a server on another computer.
  pub plain_http_allowed: bool,
  pub added_at: Timestamp,
  /// Who is signed in there from this computer: kept while the token is valid, and after the
  /// server stopped accepting it until the person signs in again or signs out.
  pub account: Option<ServerAccount>,
}

/// The account signed in to a server from this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerAccount {
  pub user: UserId,
  pub username: String,
  pub display_name: String,
  pub admin: bool,
  pub signed_in_at: Timestamp,
}

impl AppDb {
  /// The servers, in the order they were added.
  pub fn servers(&self) -> Result<Vec<StoredServer>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT id, url, name, plain_http_allowed, added_at, user_id, username, display_name, admin, signed_in_at \
       FROM servers ORDER BY added_at, id",
    )?;
    let rows = statement
      .query_map([], ServerRow::read)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(ServerRow::into_server).collect()
  }

  /// Adds `server`, with its account if it has one. An address already in the list is refused by
  /// the table.
  pub fn add_server(&mut self, server: &StoredServer) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute(
      "INSERT INTO servers (id, url, name, plain_http_allowed, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
      params![
        server.id.to_string(),
        server.url,
        server.name,
        server.plain_http_allowed,
        server.added_at.unix_millis(),
      ],
    )?;
    update_account(&transaction, server.id, server.account.as_ref())?;
    transaction.commit()?;
    Ok(())
  }

  /// Records who is signed in to `server` from this computer; `None` after signing out.
  pub fn set_server_account(&mut self, server: ServerId, account: Option<&ServerAccount>) -> Result<(), StoreError> {
    update_account(&self.connection, server, account)
  }

  /// Records the name the server gives itself now.
  pub fn set_server_name(&mut self, server: ServerId, name: &str) -> Result<(), StoreError> {
    let changed = self.connection.execute(
      "UPDATE servers SET name = ?2 WHERE id = ?1",
      params![server.to_string(), name],
    )?;
    if changed == 0 {
      return Err(StoreError::ServerNotFound(server));
    }
    Ok(())
  }
}

fn update_account(
  connection: &rusqlite::Connection,
  server: ServerId,
  account: Option<&ServerAccount>,
) -> Result<(), StoreError> {
  let changed = connection.execute(
    "UPDATE servers SET user_id = ?2, username = ?3, display_name = ?4, admin = ?5, signed_in_at = ?6 WHERE id = ?1",
    params![
      server.to_string(),
      account.map(|account| account.user.to_string()),
      account.map(|account| account.username.as_str()),
      account.map(|account| account.display_name.as_str()),
      account.map(|account| account.admin),
      account.map(|account| account.signed_in_at.unix_millis()),
    ],
  )?;
  if changed == 0 {
    return Err(StoreError::ServerNotFound(server));
  }
  Ok(())
}

/// The columns of a signed-in account, before its values are checked.
type AccountColumns = (String, String, String, bool, i64);

/// A `servers` row as stored, before its values are checked.
struct ServerRow {
  id: String,
  url: String,
  name: String,
  plain_http_allowed: bool,
  added_at: i64,
  account: Option<AccountColumns>,
}

impl ServerRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    let user: Option<String> = row.get(5)?;
    let account = match user {
      Some(user) => Some((user, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?)),
      None => None,
    };
    Ok(Self {
      id: row.get(0)?,
      url: row.get(1)?,
      name: row.get(2)?,
      plain_http_allowed: row.get(3)?,
      added_at: row.get(4)?,
      account,
    })
  }

  fn into_server(self) -> Result<StoredServer, StoreError> {
    let account = match self.account {
      Some((user, username, display_name, admin, signed_in_at)) => Some(ServerAccount {
        user: parse("server user id", &user)?,
        username,
        display_name,
        admin,
        signed_in_at: Timestamp::from_unix_millis(signed_in_at),
      }),
      None => None,
    };
    Ok(StoredServer {
      id: parse("server id", &self.id)?,
      url: self.url,
      name: self.name,
      plain_http_allowed: self.plain_http_allowed,
      added_at: Timestamp::from_unix_millis(self.added_at),
      account,
    })
  }
}
