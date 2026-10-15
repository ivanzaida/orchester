use orchester_api::{DeviceKind, ErrorCode, JoinedVia, User};
use orchester_core::id::{SessionId, UserId};
use rusqlite::{Connection, OptionalExtension, params};

use super::{
  audit,
  rows::{USER_COLUMNS, user_row},
};
use crate::{
  clock::{ADMIN_WEB_MS, DEVICE_IDLE_MS},
  error::ServerError,
};

/// A request's device token, resolved to its account.
#[derive(Clone)]
pub(crate) struct Authenticated {
  pub user: User,
  pub session: SessionId,
  pub last_used_at: i64,
  pub kind: DeviceKind,
}

/// A token that is not revoked and has not expired at `?2` (an app's 90 days after its last use, an admin sign-in's
/// 12 hours after it began), given `?2` = now − 90 days and `?3` = now − 12 hours.
const LIVE: &str = "revoked_at IS NULL AND CASE kind WHEN 'admin_web' THEN created_at>?3 ELSE last_used_at>?2 END";

pub(crate) fn authenticate(connection: &Connection, hash: &[u8], now: i64) -> Result<Authenticated, ServerError> {
  let session: Option<String> = connection
    .query_row(
      &format!("SELECT id FROM sessions WHERE token_hash=?1 AND {LIVE}"),
      params![hash, now - DEVICE_IDLE_MS, now - ADMIN_WEB_MS],
      |row| row.get(0),
    )
    .optional()?;
  let session = session
    .ok_or(ErrorCode::Unauthorized)?
    .parse()
    .map_err(|_| ServerError::StoredData)?;
  session_auth(connection, session, now)
}

/// Last use is written only when it moved by more than a minute.
pub(crate) fn touch_session(
  connection: &Connection,
  auth: &Authenticated,
  now: i64,
) -> Result<Authenticated, ServerError> {
  let mut current = checked(connection, auth, now, false)?;
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
  let row: Option<(String, i64, String)> = connection
    .query_row(
      &format!("SELECT user_id,last_used_at,kind FROM sessions WHERE id=?1 AND {LIVE}"),
      params![session.to_string(), now - DEVICE_IDLE_MS, now - ADMIN_WEB_MS],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .optional()?;
  let (user_id, last_used_at, kind) = row.ok_or(ErrorCode::Unauthorized)?;
  let user = connection.query_row(
    &format!("SELECT {USER_COLUMNS} FROM users WHERE id=?1"),
    [user_id],
    user_row,
  )?;
  if user.disabled {
    return Err(ErrorCode::Unauthorized.into());
  }
  Ok(Authenticated {
    user,
    session,
    last_used_at,
    kind: parse_kind(&kind)?,
  })
}

/// The request's device token as it is now, inside the caller's transaction; `admin` requires an admin account.
pub(crate) fn checked(
  connection: &Connection,
  auth: &Authenticated,
  now: i64,
  admin: bool,
) -> Result<Authenticated, ServerError> {
  let current = session_auth(connection, auth.session, now)?;
  if admin && !current.user.admin {
    return Err(ErrorCode::Forbidden.into());
  }
  Ok(current)
}

/// Records a device token, stored as its SHA-256 `hash`.
pub(crate) struct NewSession<'a> {
  pub id: SessionId,
  pub user: UserId,
  pub hash: &'a [u8],
  pub device: &'a str,
  pub via: JoinedVia,
  /// The cloud signing key of the identity token it was issued for.
  pub kid: Option<&'a str>,
  pub kind: DeviceKind,
}

pub(crate) fn insert_session(connection: &Connection, session: &NewSession<'_>, now: i64) -> Result<(), ServerError> {
  connection.execute(
    "INSERT INTO sessions(id,user_id,token_hash,device,created_at,last_used_at,via,kid,kind)
      VALUES (?1,?2,?3,?4,?5,?5,?6,?7,?8)",
    params![
      session.id.to_string(),
      session.user.to_string(),
      session.hash,
      session.device,
      now,
      via_name(session.via),
      session.kid,
      kind_name(session.kind)
    ],
  )?;
  Ok(())
}

pub(crate) const fn kind_name(kind: DeviceKind) -> &'static str {
  match kind {
    DeviceKind::App => "app",
    DeviceKind::AdminWeb => "admin_web",
  }
}

fn parse_kind(value: &str) -> Result<DeviceKind, ServerError> {
  [DeviceKind::App, DeviceKind::AdminWeb]
    .into_iter()
    .find(|kind| kind_name(*kind) == value)
    .ok_or(ServerError::StoredData)
}

pub(crate) const fn via_name(via: JoinedVia) -> &'static str {
  match via {
    JoinedVia::Cloud => "cloud",
    JoinedVia::Code => "code",
    JoinedVia::Before3 => "before_3",
  }
}

pub(crate) fn parse_via(value: &str) -> Result<JoinedVia, ServerError> {
  [JoinedVia::Cloud, JoinedVia::Code, JoinedVia::Before3]
    .into_iter()
    .find(|via| via_name(*via) == value)
    .ok_or(ServerError::StoredData)
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
