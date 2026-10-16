use orchester_api::{AdminSession, AdminUser, ErrorCode, SessionOwner, User, UserChanges};
use orchester_core::id::{SessionId, UserId};
use rusqlite::{Row, params};

use super::Server;
use crate::{
  clock::DEVICE_IDLE_MS,
  db::{self, Authenticated},
  error::ServerError,
};

impl Server {
  pub(crate) async fn users(&self, auth: Authenticated) -> Result<Vec<AdminUser>, ServerError> {
    let now = self.now();
    self
      .inner
      .db
      .read(move |db| {
        db::checked(db, &auth, now, true)?;
        let mut query = db.prepare(&format!(
          "SELECT {}, (SELECT COUNT(*) FROM memberships WHERE user_id=users.id) FROM users ORDER BY username",
          db::USER_COLUMNS
        ))?;
        let users = query
          .query_map([], |row| {
            Ok(AdminUser {
              user: db::user_row(row)?,
              projects: row.get(6)?,
            })
          })?
          .collect::<Result<_, _>>()?;
        Ok(users)
      })
      .await
  }

  /// Every computer joined to the server, newest use first; browsers signed in to the admin are not computers.
  pub(crate) async fn admin_sessions(&self, auth: Authenticated) -> Result<Vec<AdminSession>, ServerError> {
    self
      .snapshot(move |db, now| {
        let current = db::checked(db, &auth, now, true)?;
        let mut query = db.prepare(
          "SELECT s.id,s.device,s.via,s.created_at,s.last_used_at,u.id,u.username,u.display_name
            FROM sessions s JOIN users u ON u.id=s.user_id
            WHERE s.kind='app' AND s.revoked_at IS NULL AND s.last_used_at>?1 AND u.disabled_at IS NULL
            ORDER BY s.last_used_at DESC,s.id",
        )?;
        let rows = query
          .query_map(params![now - DEVICE_IDLE_MS], session_row)?
          .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(|row| row.session(current.session)).collect()
      })
      .await
  }

  /// Signs one computer out, whoever's it is.
  pub(crate) async fn revoke_admin_session(&self, auth: Authenticated, id: SessionId) -> Result<(), ServerError> {
    self
      .account_write(move |db, now| {
        let current = db::checked(db, &auth, now, true)?;
        let owner: String = db
          .query_row(
            "SELECT user_id FROM sessions WHERE id=?1 AND kind='app' AND revoked_at IS NULL",
            [id.to_string()],
            |row| row.get(0),
          )
          .map_err(|_| ErrorCode::NotFound)?;
        let owner = owner.parse().map_err(|_| ServerError::StoredData)?;
        db::revoke_session(db, owner, id, current.user.id, now)
      })
      .await
  }

  pub(crate) async fn update_user(
    &self,
    auth: Authenticated,
    id: UserId,
    changes: UserChanges,
  ) -> Result<User, ServerError> {
    let now = self.now();
    self
      .inner
      .db
      .write(move |tx| {
        let current = db::checked(tx, &auth, now, true)?;
        db::patch_user(tx, id, &changes, current.user.id, now)
      })
      .await
  }

  pub(crate) async fn revoke_user_sessions(&self, auth: Authenticated, id: UserId) -> Result<(), ServerError> {
    let now = self.now();
    self
      .inner
      .db
      .write(move |tx| {
        let current = db::checked(tx, &auth, now, true)?;
        if db::user_by_id(tx, id)?.is_none() {
          return Err(ErrorCode::NotFound.into());
        }
        db::revoke_sessions(tx, id, None, Some(current.user.id), now)
      })
      .await
  }
}

/// A computer's row as stored, before its ids and how it joined are parsed.
struct SessionRow {
  id: String,
  device: String,
  via: String,
  created_at: i64,
  last_used_at: i64,
  user: (String, String, String),
}

fn session_row(row: &Row<'_>) -> rusqlite::Result<SessionRow> {
  Ok(SessionRow {
    id: row.get(0)?,
    device: row.get(1)?,
    via: row.get(2)?,
    created_at: row.get(3)?,
    last_used_at: row.get(4)?,
    user: (row.get(5)?, row.get(6)?, row.get(7)?),
  })
}

impl SessionRow {
  fn session(self, current: SessionId) -> Result<AdminSession, ServerError> {
    let id: SessionId = self.id.parse().map_err(|_| ServerError::StoredData)?;
    let (user, username, display_name) = self.user;
    Ok(AdminSession {
      id,
      device: self.device,
      user: SessionOwner {
        id: user.parse().map_err(|_| ServerError::StoredData)?,
        username,
        display_name,
      },
      via: db::parse_via(&self.via)?,
      created_at: self.created_at,
      last_used_at: self.last_used_at,
      expires_at: self.last_used_at + DEVICE_IDLE_MS,
      current: id == current,
    })
  }
}
