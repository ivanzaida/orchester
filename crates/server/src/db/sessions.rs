use orchester_api::{ErrorCode, User};
use orchester_core::id::{SessionId, UserId};
use rusqlite::{Connection, OptionalExtension, params};

use super::{
  audit,
  rows::{USER_COLUMNS, user_row},
};
use crate::{clock::SESSION_IDLE_MS, error::ServerError};

#[derive(Clone)]
pub(crate) struct Authenticated {
  pub user: User,
  pub session: SessionId,
  pub must_change: bool,
  pub last_used_at: i64,
}

pub(crate) fn authenticate(connection: &Connection, hash: &[u8], now: i64) -> Result<Authenticated, ServerError> {
  let session: Option<String> = connection
    .query_row(
      "SELECT id FROM sessions WHERE token_hash=?1 AND revoked_at IS NULL AND last_used_at>?2",
      params![hash, now - SESSION_IDLE_MS],
      |row| row.get(0),
    )
    .optional()?;
  let session = session
    .ok_or(ErrorCode::Unauthorized)?
    .parse()
    .map_err(|_| ServerError::StoredData)?;
  session_auth(connection, session, now)
}

pub(crate) fn touch_session(
  connection: &Connection,
  auth: &Authenticated,
  now: i64,
) -> Result<Authenticated, ServerError> {
  let mut current = checked(connection, auth, now, true, false)?;
  if now - current.last_used_at <= 60_000 {
    return Ok(current);
  }
  connection.execute(
    "UPDATE sessions SET last_used_at=?1 WHERE id=?2",
    params![now, current.session.to_string()],
  )?;
  current.last_used_at = now;
  Ok(current)
}

fn session_auth(connection: &Connection, session: SessionId, now: i64) -> Result<Authenticated, ServerError> {
  let row: Option<(String, bool, i64)> = connection
    .query_row(
      "SELECT user_id,initial_password,last_used_at FROM sessions WHERE id=?1 AND revoked_at IS NULL AND last_used_at>?2",
      params![session.to_string(), now - SESSION_IDLE_MS],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .optional()?;
  let (user_id, initial, last_used_at) = row.ok_or(ErrorCode::Unauthorized)?;
  let user = connection.query_row(
    &format!("SELECT {USER_COLUMNS} FROM users WHERE id=?1"),
    [user_id],
    user_row,
  )?;
  if user.user.disabled {
    return Err(ErrorCode::Unauthorized.into());
  }
  Ok(Authenticated {
    user: user.user,
    session,
    must_change: initial || user.must_change,
    last_used_at,
  })
}

pub(crate) fn checked(
  connection: &Connection,
  auth: &Authenticated,
  now: i64,
  allow_initial: bool,
  admin: bool,
) -> Result<Authenticated, ServerError> {
  let current = session_auth(connection, auth.session, now)?;
  if current.must_change && !allow_initial {
    return Err(ErrorCode::PasswordChangeRequired.into());
  }
  if admin && !current.user.admin {
    return Err(ErrorCode::Forbidden.into());
  }
  Ok(current)
}

pub(crate) fn revoke_session(
  connection: &Connection,
  user: UserId,
  session: SessionId,
  actor: UserId,
  now: i64,
) -> Result<(), ServerError> {
  let exists = connection.execute(
    "UPDATE sessions SET revoked_at=?1 WHERE id=?2 AND user_id=?3 AND revoked_at IS NULL",
    params![now, session.to_string(), user.to_string()],
  )?;
  if exists == 0 {
    return Err(ErrorCode::NotFound.into());
  }
  audit(
    connection,
    Some(actor),
    "session_revoked",
    &session.to_string(),
    None,
    now,
  )
}

pub(crate) fn revoke_sessions(
  connection: &Connection,
  user: UserId,
  keep: Option<SessionId>,
  actor: Option<UserId>,
  now: i64,
) -> Result<(), ServerError> {
  let mut statement = connection.prepare("SELECT id FROM sessions WHERE user_id=?1 AND revoked_at IS NULL")?;
  let sessions = statement
    .query_map([user.to_string()], |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  for id in sessions {
    if keep.is_some_and(|keep| keep.to_string() == id) {
      continue;
    }
    connection.execute("UPDATE sessions SET revoked_at=?1 WHERE id=?2", params![now, id])?;
    audit(connection, actor, "session_revoked", &id, None, now)?;
  }
  Ok(())
}
