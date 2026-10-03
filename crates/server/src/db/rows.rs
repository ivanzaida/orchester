use orchester_api::User;
use orchester_core::id::UserId;
use rusqlite::{Row, types::Type};

use crate::passwords::PasswordHash;

pub(crate) struct StoredUser {
  pub user: User,
  pub hash: PasswordHash,
  pub must_change: bool,
}

pub(crate) fn user_row(row: &Row<'_>) -> rusqlite::Result<StoredUser> {
  let id: String = row.get(0)?;
  let id: UserId = id
    .parse()
    .map_err(|error| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error)))?;
  Ok(StoredUser {
    user: User {
      id,
      username: row.get(1)?,
      display_name: row.get(2)?,
      admin: row.get(3)?,
      disabled: row.get(4)?,
    },
    hash: PasswordHash::stored(row.get(5)?),
    must_change: row.get(6)?,
  })
}

pub(crate) const USER_COLUMNS: &str =
  "id,username,display_name,admin,disabled_at IS NOT NULL,password_hash,must_change_password";
