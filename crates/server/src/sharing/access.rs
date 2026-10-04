use orchester_api::{ErrorCode, MemberRole, UserRef};
use orchester_core::id::{ProjectId, UserId};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
  db::{self, Authenticated},
  error::ServerError,
};

pub fn user_ref(db: &Connection, id: UserId) -> Result<UserRef, ServerError> {
  Ok(UserRef {
    id,
    display_name: db.query_row("SELECT display_name FROM users WHERE id=?1", [id.to_string()], |row| {
      row.get(0)
    })?,
  })
}

pub fn role(db: &Connection, project: ProjectId, user: UserId) -> Result<Option<MemberRole>, ServerError> {
  let role: Option<String> = db
    .query_row(
      "SELECT role FROM memberships WHERE project=?1 AND user_id=?2",
      params![project.to_string(), user.to_string()],
      |row| row.get(0),
    )
    .optional()?;
  role
    .map(|role| match role.as_str() {
      "owner" => Ok(MemberRole::Owner),
      "member" => Ok(MemberRole::Member),
      _ => Err(ServerError::StoredData),
    })
    .transpose()
}
pub const fn role_name(role: MemberRole) -> &'static str {
  match role {
    MemberRole::Owner => "owner",
    MemberRole::Member => "member",
  }
}

pub fn member(
  db: &Connection,
  auth: &Authenticated,
  project: ProjectId,
  now: i64,
) -> Result<Authenticated, ServerError> {
  let current = db::checked(db, auth, now, false, false)?;
  if role(db, project, current.user.id)?.is_none() {
    return Err(ErrorCode::NotFound.into());
  }
  Ok(current)
}

pub fn owner(
  db: &Connection,
  auth: &Authenticated,
  project: ProjectId,
  now: i64,
) -> Result<Authenticated, ServerError> {
  let current = db::checked(db, auth, now, false, false)?;
  if current.user.admin {
    let exists: bool = db.query_row(
      "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
      [project.to_string()],
      |row| row.get(0),
    )?;
    if !exists {
      return Err(ErrorCode::NotFound.into());
    }
    return Ok(current);
  }
  match role(db, project, current.user.id)? {
    Some(MemberRole::Owner) => Ok(current),
    Some(MemberRole::Member) => Err(ErrorCode::Forbidden.into()),
    None => Err(ErrorCode::NotFound.into()),
  }
}

pub fn owners(db: &Connection, project: ProjectId) -> Result<Vec<UserRef>, ServerError> {
  let mut query =
    db.prepare("SELECT user_id FROM memberships WHERE project=?1 AND role='owner' ORDER BY added_at,user_id")?;
  let ids = query
    .query_map([project.to_string()], |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  ids
    .into_iter()
    .map(|id| user_ref(db, id.parse().map_err(|_| ServerError::StoredData)?))
    .collect()
}
