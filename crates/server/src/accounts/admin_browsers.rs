//! Browsers signed in to the server admin, which admins see and sign out one by one (security review 1, minor 5).

use orchester_api::{AdminBrowser, ErrorCode, SessionOwner};
use orchester_core::id::SessionId;
use rusqlite::{Row, params};

use super::Server;
use crate::{
  clock::ADMIN_WEB_MS,
  db::{self, Authenticated},
  error::ServerError,
};

impl Server {
  /// Every live admin sign-in in a browser, newest first.
  pub(crate) async fn admin_browsers(&self, auth: Authenticated) -> Result<Vec<AdminBrowser>, ServerError> {
    self
      .snapshot(move |db, now| {
        let current = db::checked(db, &auth, now, true)?;
        let mut query = db.prepare(
          "SELECT s.id,s.via,s.created_at,s.last_used_at,u.id,u.username,u.display_name
            FROM sessions s JOIN users u ON u.id=s.user_id
            WHERE s.kind='admin_web' AND s.revoked_at IS NULL AND s.created_at>?1 AND u.disabled_at IS NULL
            ORDER BY s.created_at DESC,s.id",
        )?;
        let rows = query
          .query_map(params![now - ADMIN_WEB_MS], browser_row)?
          .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(|row| row.browser(current.session)).collect()
      })
      .await
  }

  /// Signs one browser out of the admin, whoever's it is.
  pub(crate) async fn revoke_admin_browser(&self, auth: Authenticated, id: SessionId) -> Result<(), ServerError> {
    self
      .account_write(move |db, now| {
        let current = db::checked(db, &auth, now, true)?;
        let owner: String = db
          .query_row(
            "SELECT user_id FROM sessions WHERE id=?1 AND kind='admin_web' AND revoked_at IS NULL",
            [id.to_string()],
            |row| row.get(0),
          )
          .map_err(|_| ErrorCode::NotFound)?;
        let owner = owner.parse().map_err(|_| ServerError::StoredData)?;
        db::revoke_session(db, owner, id, current.user.id, now)
      })
      .await
  }
}

/// A browser's row as stored, before its ids and how it signed in are parsed.
struct BrowserRow {
  id: String,
  via: String,
  created_at: i64,
  last_used_at: i64,
  user: (String, String, String),
}

fn browser_row(row: &Row<'_>) -> rusqlite::Result<BrowserRow> {
  Ok(BrowserRow {
    id: row.get(0)?,
    via: row.get(1)?,
    created_at: row.get(2)?,
    last_used_at: row.get(3)?,
    user: (row.get(4)?, row.get(5)?, row.get(6)?),
  })
}

impl BrowserRow {
  fn browser(self, current: SessionId) -> Result<AdminBrowser, ServerError> {
    let id: SessionId = self.id.parse().map_err(|_| ServerError::StoredData)?;
    let (user, username, display_name) = self.user;
    Ok(AdminBrowser {
      id,
      user: SessionOwner {
        id: user.parse().map_err(|_| ServerError::StoredData)?,
        username,
        display_name,
      },
      via: db::parse_via(&self.via)?,
      created_at: self.created_at,
      last_used_at: self.last_used_at,
      expires_at: self.created_at + ADMIN_WEB_MS,
      current: id == current,
    })
  }
}
