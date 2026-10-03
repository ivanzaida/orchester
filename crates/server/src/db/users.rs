use orchester_api::{CreateUser, ErrorCode, User, UserChanges};
use orchester_core::id::UserId;
use rusqlite::{Connection, OptionalExtension, params};

use super::{
  audit, revoke_sessions,
  rows::{StoredUser, USER_COLUMNS, user_row},
};
use crate::{error::ServerError, passwords::PasswordHash};

pub(crate) fn validate_user(input: &CreateUser) -> Result<(), ServerError> {
  // ASCII usernames give SQLite NOCASE and the rate limiter the same identity.
  if input.username.is_empty()
    || input.username.len() > 64
    || !input
      .username
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    || input.display_name.trim().is_empty()
    || input.display_name.chars().count() > 256
  {
    return Err(ErrorCode::Invalid.into());
  }
  Ok(())
}

pub(crate) fn find_user(connection: &Connection, username: &str) -> Result<Option<StoredUser>, ServerError> {
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

pub(crate) fn create_user(
  connection: &Connection,
  input: &CreateUser,
  hash: &PasswordHash,
  actor: Option<UserId>,
  now: i64,
) -> Result<User, ServerError> {
  validate_user(input)?;
  if find_user(connection, &input.username)?.is_some() {
    return Err(ErrorCode::UsernameTaken.into());
  }
  let user = User {
    id: UserId::generate(),
    username: input.username.clone(),
    display_name: input.display_name.clone(),
    admin: input.admin,
    disabled: false,
  };
  connection.execute("INSERT INTO users(id,username,display_name,password_hash,admin,must_change_password,created_at,password_changed_at)
    VALUES (?1,?2,?3,?4,?5,1,?6,?6)", params![user.id.to_string(), user.username, user.display_name, hash.expose(), user.admin, now])?;
  audit(connection, actor, "user_created", &user.id.to_string(), None, now)?;
  Ok(user)
}

pub(crate) fn patch_user(
  connection: &Connection,
  id: UserId,
  changes: &UserChanges,
  actor: UserId,
  now: i64,
) -> Result<User, ServerError> {
  let stored = connection
    .query_row(
      &format!("SELECT {USER_COLUMNS} FROM users WHERE id=?1"),
      [id.to_string()],
      user_row,
    )
    .optional()?
    .ok_or(ErrorCode::NotFound)?;
  let mut user = stored.user;
  let original = user.clone();
  if let Some(name) = &changes.display_name {
    if name.trim().is_empty() || name.chars().count() > 256 {
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

pub(crate) fn reset_password(
  connection: &Connection,
  id: UserId,
  hash: &PasswordHash,
  actor: Option<UserId>,
  now: i64,
) -> Result<(), ServerError> {
  let changed = connection.execute(
    "UPDATE users SET password_hash=?1,must_change_password=1,password_changed_at=?2 WHERE id=?3",
    params![hash.expose(), now, id.to_string()],
  )?;
  if changed == 0 {
    return Err(ErrorCode::NotFound.into());
  }
  revoke_sessions(connection, id, None, actor, now)?;
  audit(connection, actor, "password_reset", &id.to_string(), None, now)
}
