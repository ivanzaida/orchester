//! The server's own settings in its database (`/server/settings`): its name, which an admin may change from the one
//! `server.toml` gives, and join approval (`approvals`).

use orchester_api::ErrorCode;
use orchester_core::id::UserId;
use rusqlite::Connection;

use crate::{db, error::ServerError};

/// The longest name, in characters, a server may have.
const MAX_NAME_CHARS: usize = 128;

/// The server's name: the one an admin set, or `configured` (`name` in `server.toml`).
pub(crate) fn name(db: &Connection, configured: &str) -> Result<String, ServerError> {
  let stored: Option<String> = db.query_row("SELECT name FROM settings WHERE id=1", [], |row| row.get(0))?;
  Ok(stored.unwrap_or_else(|| configured.to_owned()))
}

/// Renames the server; a blank or overlong name is refused.
pub(crate) fn set_name(db: &Connection, name: &str, actor: UserId, now: i64) -> Result<(), ServerError> {
  let name = name.trim();
  if name.is_empty() || name.chars().count() > MAX_NAME_CHARS || name.chars().any(char::is_control) {
    return Err(ErrorCode::Invalid.into());
  }
  db.execute("UPDATE settings SET name=?1 WHERE id=1", [name])?;
  db::audit(db, Some(actor), "server_renamed", "settings", None, now)
}
