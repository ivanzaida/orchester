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
    .map(|role| parse_role(&role).ok_or(ServerError::StoredData))
    .transpose()
}

pub fn parse_role(role: &str) -> Option<MemberRole> {
  match role {
    "owner" => Some(MemberRole::Owner),
    "member" => Some(MemberRole::Member),
    _ => None,
  }
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
  let current = db::checked(db, auth, now, false)?;
  if role(db, project, current.user.id)?.is_none() {
    return Err(ErrorCode::NotFound.into());
  }
  Ok(current)
}

/// A member of `project`, or an admin: who may read its members.
pub fn viewer(
  db: &Connection,
  auth: &Authenticated,
  project: ProjectId,
  now: i64,
) -> Result<Authenticated, ServerError> {
  let current = db::checked(db, auth, now, false)?;
  if current.user.admin {
    return owner(db, auth, project, now);
  }
  member(db, auth, project, now)
}

pub fn owner(
  db: &Connection,
  auth: &Authenticated,
  project: ProjectId,
  now: i64,
) -> Result<Authenticated, ServerError> {
  let current = db::checked(db, auth, now, false)?;
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
