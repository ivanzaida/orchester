use orchester_api::User;
use orchester_core::id::UserId;
use rusqlite::{Row, types::Type};

pub(crate) fn user_row(row: &Row<'_>) -> rusqlite::Result<User> {
  let id: String = row.get(0)?;
  let id: UserId = id
    .parse()
    .map_err(|error| rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(error)))?;
  Ok(User {
    id,
    username: row.get(1)?,
    display_name: row.get(2)?,
    admin: row.get(3)?,
    disabled: row.get(4)?,
    cloud: row.get(5)?,
  })
}

pub(crate) const USER_COLUMNS: &str =
  "id,username,display_name,admin,disabled_at IS NOT NULL,cloud_subject IS NOT NULL";
