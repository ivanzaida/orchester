//! The shared servers the person added (`docs/architecture/server.md`, "Desktop data"): each one's
//! address, name, whether plain HTTP is allowed for it, and the account signed in there. The token
//! of that account is in the OS keychain, never here. Whose each server is, and adding and listing them by that, is in
//! [`super::server_owners`].

use orchester_core::{
  id::{ServerId, TeamServerId, UserId},
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

/// API 3 (`docs/architecture/server.md`, row 39): the team server's own certificate, pinned by its SHA-256.
pub(super) const PIN_MIGRATION: &str = "ALTER TABLE servers ADD COLUMN certificate_pin TEXT;";

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
  /// The SHA-256 of the certificate this computer trusts for the server, in lowercase hex, when it pinned the
  /// server's own certificate; `None` when the operating system's trust store decides.
  pub certificate_pin: Option<String>,
  pub added_at: Timestamp,
  /// How this computer last joined the server; `None` for a server added before API 3.
  pub joined_with: Option<JoinedWith>,
  /// Who is signed in there from this computer: kept while the token is valid, and after the
  /// server stopped accepting it until the person signs in again or signs out.
  pub account: Option<ServerAccount>,
}

/// How this computer joined a team server (`docs/architecture/server.md`, rows 42 and 45).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinedWith {
  /// A setup, invite, account or pairing code.
  Code,
  /// The person's Orchester Cloud identity, for the server the cloud knows by this id.
  Cloud(TeamServerId),
}

impl JoinedWith {
  pub(super) fn columns(self) -> (&'static str, Option<String>) {
    match self {
      Self::Code => ("code", None),
      Self::Cloud(server) => ("cloud", Some(server.to_string())),
    }
  }
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
  /// Records who is signed in to `server` from this computer; `None` after signing out.
  pub fn set_server_account(&mut self, server: ServerId, account: Option<&ServerAccount>) -> Result<(), StoreError> {
    update_account(&self.connection, server, account)
  }

  /// Records how this computer trusts `server`: `pin` (hex SHA-256 of its certificate) when it pinned the server's own
  /// certificate, otherwise the system's trust store with `plain_http` allowed or not.
  pub fn set_server_trust(&mut self, server: ServerId, pin: Option<&str>, plain_http: bool) -> Result<(), StoreError> {
    let changed = self.connection.execute(
      "UPDATE servers SET certificate_pin = ?2, plain_http_allowed = ?3 WHERE id = ?1",
      params![server.to_string(), pin, plain_http],
    )?;
    if changed == 0 {
      return Err(StoreError::ServerNotFound(server));
    }
    Ok(())
  }

  /// Records how this computer joined `server` last.
  pub fn set_server_joined_with(&mut self, server: ServerId, joined_with: JoinedWith) -> Result<(), StoreError> {
    let (kind, team_server) = joined_with.columns();
    let changed = self.connection.execute(
      "UPDATE servers SET joined_with = ?2, team_server = ?3 WHERE id = ?1",
      params![server.to_string(), kind, team_server],
    )?;
    if changed == 0 {
      return Err(StoreError::ServerNotFound(server));
    }
    Ok(())
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

pub(super) fn update_account(
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

/// The columns [`ServerRow::read`] reads, in its order.
pub(super) const COLUMNS: &str = "SELECT id, url, name, plain_http_allowed, added_at, user_id, username, display_name,   admin, signed_in_at, certificate_pin, joined_with, team_server FROM servers";

/// A `servers` row as stored, before its values are checked.
pub(super) struct ServerRow {
  id: String,
  url: String,
  name: String,
  plain_http_allowed: bool,
  added_at: i64,
  account: Option<AccountColumns>,
  certificate_pin: Option<String>,
  joined_with: Option<String>,
  team_server: Option<String>,
}

impl ServerRow {
  pub(super) fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
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
      certificate_pin: row.get(10)?,
      joined_with: row.get(11)?,
      team_server: row.get(12)?,
    })
  }

  pub(super) fn into_server(self) -> Result<StoredServer, StoreError> {
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
    let joined_with = match (self.joined_with.as_deref(), self.team_server) {
      (None, _) => None,
      (Some("code"), _) => Some(JoinedWith::Code),
      (Some("cloud"), Some(server)) => Some(JoinedWith::Cloud(parse("team server id", &server)?)),
      (Some(other), _) => return Err(crate::rows::corrupt("server join", other)),
    };
    Ok(StoredServer {
      id: parse("server id", &self.id)?,
      url: self.url,
      name: self.name,
      plain_http_allowed: self.plain_http_allowed,
      certificate_pin: self.certificate_pin,
      added_at: Timestamp::from_unix_millis(self.added_at),
      joined_with,
      account,
    })
  }
}
