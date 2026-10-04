use orchester_api::{ErrorCode, Member, MemberCreate, MemberRole};
use orchester_core::id::{ProjectId, UserId};
use rusqlite::{Connection, OptionalExtension, params};

use super::{access, claims, events};
use crate::{db, error::ServerError};

pub fn list(db: &Connection, project: ProjectId) -> Result<Vec<Member>, ServerError> {
  let mut query =
    db.prepare("SELECT user_id,role,added_at FROM memberships WHERE project=?1 ORDER BY added_at,user_id")?;
  let rows = query
    .query_map([project.to_string()], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, i64>(2)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  rows
    .into_iter()
    .map(|(id, role, added_at)| {
      Ok(Member {
        user: access::user_ref(db, id.parse().map_err(|_| ServerError::StoredData)?)?,
        role: match role.as_str() {
          "owner" => MemberRole::Owner,
          "member" => MemberRole::Member,
          _ => return Err(ServerError::StoredData),
        },
        added_at,
      })
    })
    .collect()
}

pub fn add(
  db: &Connection,
  project: ProjectId,
  input: &MemberCreate,
  actor: UserId,
  now: i64,
) -> Result<Member, ServerError> {
  let user = db::find_user(db, &input.username)?
    .filter(|user| !user.user.disabled)
    .ok_or(ErrorCode::NoSuchUser)?
    .user
    .id;
  if access::role(db, project, user)?.is_some() {
    return Err(ErrorCode::AlreadyMember.into());
  }
  db.execute(
    "INSERT INTO memberships VALUES (?1,?2,?3,?4,?5,?6)",
    params![
      project.to_string(),
      user.to_string(),
      access::role_name(input.role),
      actor.to_string(),
      now,
      orchester_core::id::IdempotencyKey::generate().to_string()
    ],
  )?;
  audit(db, project, user, actor, "member_added", now)?;
  Ok(Member {
    user: access::user_ref(db, user)?,
    role: input.role,
    added_at: now,
  })
}

pub fn change(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  role: MemberRole,
  actor: UserId,
  now: i64,
) -> Result<Member, ServerError> {
  let mut member = list(db, project)?
    .into_iter()
    .find(|member| member.user.id == user)
    .ok_or(ErrorCode::NotFound)?;
  if member.role == role {
    return Ok(member);
  }
  if member.role == MemberRole::Owner {
    check_last_owner(db, project)?;
  }
  db.execute(
    "UPDATE memberships SET role=?1 WHERE project=?2 AND user_id=?3",
    params![access::role_name(role), project.to_string(), user.to_string()],
  )?;
  member.role = role;
  audit(db, project, user, actor, "member_role_changed", now)?;
  Ok(member)
}

pub fn remove(db: &Connection, project: ProjectId, user: UserId, actor: UserId, now: i64) -> Result<(), ServerError> {
  let role = access::role(db, project, user)?.ok_or(ErrorCode::NotFound)?;
  if role == MemberRole::Owner {
    check_last_owner(db, project)?;
  }
  db.execute(
    "DELETE FROM memberships WHERE project=?1 AND user_id=?2",
    params![project.to_string(), user.to_string()],
  )?;
  claims::release_user(db, Some(project), user, actor, now)?;
  audit(db, project, user, actor, "member_removed", now)
}

fn check_last_owner(db: &Connection, project: ProjectId) -> Result<(), ServerError> {
  let count: i64 = db.query_row(
    "SELECT COUNT(*) FROM memberships WHERE project=?1 AND role='owner'",
    [project.to_string()],
    |row| row.get(0),
  )?;
  if count <= 1 {
    return Err(ErrorCode::LastOwner.into());
  }
  Ok(())
}

fn audit(
  db: &Connection,
  project: ProjectId,
  user: UserId,
  actor: UserId,
  action: &str,
  now: i64,
) -> Result<(), ServerError> {
  db::audit(db, Some(actor), action, &format!("{project}/{user}"), None, now)?;
  event(db, project, now)
}

pub fn event(db: &Connection, project: ProjectId, now: i64) -> Result<(), ServerError> {
  events::append(
    db,
    project,
    orchester_api::EventKind::Members,
    &orchester_api::Members {
      members: list(db, project)?,
    },
    now,
  )
}

pub fn generation(db: &Connection, project: ProjectId, user: UserId) -> Result<Option<String>, ServerError> {
  Ok(
    db.query_row(
      "SELECT generation FROM memberships WHERE project=?1 AND user_id=?2",
      params![project.to_string(), user.to_string()],
      |row| row.get(0),
    )
    .optional()?,
  )
}
