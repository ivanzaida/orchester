use orchester_api::{ErrorCode, User, UserChanges};
use orchester_core::id::UserId;
use rusqlite::{Connection, OptionalExtension, params};

use super::{
  audit, revoke_sessions,
  rows::{USER_COLUMNS, user_row},
};
use crate::error::ServerError;

/// An account to create: from the setup code, an invite code or a cloud join.
pub(crate) struct NewUser<'a> {
  pub username: &'a str,
  pub display_name: &'a str,
  pub admin: bool,
  /// The cloud account it is linked to.
  pub cloud_subject: Option<&'a str>,
}

/// ASCII usernames give SQLite NOCASE and the rate limiter the same identity.
pub(crate) fn valid_username(username: &str) -> bool {
  !username.is_empty()
    && username.len() <= 64
    && username
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

pub(crate) fn valid_display_name(name: &str) -> bool {
  !name.trim().is_empty() && name.chars().count() <= 256
}

pub(crate) fn find_user(connection: &Connection, username: &str) -> Result<Option<User>, ServerError> {
  Ok(
    connection
      .query_row(
        &format!("SELECT {USER_COLUMNS} FROM users WHERE username=?1 COLLATE NOCASE"),
        [username],
        user_row,
      )
      .optional()?,
  )
}

pub(crate) fn user_by_id(connection: &Connection, id: UserId) -> Result<Option<User>, ServerError> {
  Ok(
    connection
      .query_row(
        &format!("SELECT {USER_COLUMNS} FROM users WHERE id=?1"),
        [id.to_string()],
        user_row,
      )
      .optional()?,
  )
}

/// The account linked to the cloud account `subject`.
pub(crate) fn user_by_subject(connection: &Connection, subject: &str) -> Result<Option<User>, ServerError> {
  Ok(
    connection
      .query_row(
        &format!("SELECT {USER_COLUMNS} FROM users WHERE cloud_subject=?1"),
        [subject],
        user_row,
      )
      .optional()?,
  )
}

pub(crate) fn create_user(
  connection: &Connection,
  input: &NewUser<'_>,
  actor: Option<UserId>,
  now: i64,
) -> Result<User, ServerError> {
  if !valid_username(input.username) || !valid_display_name(input.display_name) {
    return Err(ErrorCode::Invalid.into());
  }
  if find_user(connection, input.username)?.is_some() {
    return Err(ErrorCode::UsernameTaken.into());
  }
  let user = User {
    id: UserId::generate(),
    username: input.username.to_owned(),
    display_name: input.display_name.to_owned(),
    admin: input.admin,
    disabled: false,
    cloud: input.cloud_subject.is_some(),
  };
  connection.execute(
    "INSERT INTO users(id,username,display_name,admin,created_at,cloud_subject) VALUES (?1,?2,?3,?4,?5,?6)",
    params![
      user.id.to_string(),
      user.username,
      user.display_name,
      user.admin,
      now,
      input.cloud_subject
    ],
  )?;
  audit(
    connection,
    actor.or(Some(user.id)),
    "user_created",
    &user.id.to_string(),
    None,
    now,
  )?;
  Ok(user)
}

/// Links an account to the cloud account `subject`.
pub(crate) fn link_subject(connection: &Connection, user: UserId, subject: &str, now: i64) -> Result<(), ServerError> {
  connection.execute(
    "UPDATE users SET cloud_subject=?1 WHERE id=?2",
    params![subject, user.to_string()],
  )?;
  audit(connection, Some(user), "cloud_linked", &user.to_string(), None, now)
}

pub(crate) fn patch_user(
  connection: &Connection,
  id: UserId,
  changes: &UserChanges,
  actor: UserId,
  now: i64,
) -> Result<User, ServerError> {
  let mut user = user_by_id(connection, id)?.ok_or(ErrorCode::NotFound)?;
  let original = user.clone();
  if let Some(name) = &changes.display_name {
    if !valid_display_name(name) {
      return Err(ErrorCode::Invalid.into());
    }
    user.display_name.clone_from(name);
  }
  user.admin = changes.admin.unwrap_or(user.admin);
  user.disabled = changes.disabled.unwrap_or(user.disabled);
  if user.display_name == original.display_name && user.admin == original.admin && user.disabled == original.disabled {
    return Ok(user);
  }
  if stored_active_admin(connection, id)? && (!user.admin || user.disabled) {
    let count: i64 = connection.query_row(
      "SELECT COUNT(*) FROM users WHERE admin=1 AND disabled_at IS NULL",
      [],
      |row| row.get(0),
    )?;
    if count <= 1 {
      return Err(ErrorCode::LastAdmin.into());
    }
  }
  connection.execute("UPDATE users SET display_name=?1,admin=?2,disabled_at=CASE WHEN ?3 THEN COALESCE(disabled_at,?4) ELSE NULL END WHERE id=?5",
    params![user.display_name, user.admin, user.disabled, now, id.to_string()])?;
  if user.disabled {
    revoke_sessions(connection, id, None, Some(actor), now)?;
    crate::sharing::claims::release_user(connection, None, id, actor, now)?;
  }
  audit(connection, Some(actor), "user_changed", &id.to_string(), None, now)?;
  Ok(user)
}

fn stored_active_admin(connection: &Connection, id: UserId) -> Result<bool, ServerError> {
  Ok(connection.query_row(
    "SELECT admin AND disabled_at IS NULL FROM users WHERE id=?1",
    [id.to_string()],
    |row| row.get(0),
  )?)
}

/// Whether the server has an active admin; until it has, it prints a setup code.
pub(crate) fn has_admin(connection: &Connection) -> Result<bool, ServerError> {
  Ok(connection.query_row(
    "SELECT EXISTS(SELECT 1 FROM users WHERE admin=1 AND disabled_at IS NULL)",
    [],
    |row| row.get(0),
  )?)
}
