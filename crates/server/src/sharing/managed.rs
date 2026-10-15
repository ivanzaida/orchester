//! Projects as the server admin manages them (`GET /admin/projects`): every project for an admin, the projects they
//! own for an owner who is not one, each with its members, open invites, owners and who created it.

use orchester_api::{ErrorCode, ManagedProject, MemberRole, User, UserRef};
use orchester_core::id::{ProjectId, UserId};
use rusqlite::{Connection, params};

use super::{access, projects};
use crate::error::ServerError;

/// The projects `viewer` manages, oldest first.
pub fn list(db: &Connection, viewer: &User, base: &str, now: i64) -> Result<Vec<ManagedProject>, ServerError> {
  let mut query = db.prepare(
    "SELECT id FROM projects WHERE ?1 OR id IN (SELECT project FROM memberships WHERE user_id=?2 AND role='owner')
      ORDER BY created_at,id",
  )?;
  let ids = query
    .query_map(params![viewer.admin, viewer.id.to_string()], |row| {
      row.get::<_, String>(0)
    })?
    .collect::<Result<Vec<_>, _>>()?;
  ids
    .into_iter()
    .map(|id| {
      load(
        db,
        id.parse().map_err(|_| ServerError::StoredData)?,
        viewer.id,
        base,
        now,
      )
    })
    .collect()
}

/// One project `viewer` manages; `404` for any other.
pub fn show(
  db: &Connection,
  viewer: &User,
  project: ProjectId,
  base: &str,
  now: i64,
) -> Result<ManagedProject, ServerError> {
  if !viewer.admin && access::role(db, project, viewer.id)? != Some(MemberRole::Owner) {
    return Err(ErrorCode::NotFound.into());
  }
  load(db, project, viewer.id, base, now)
}

fn load(
  db: &Connection,
  project: ProjectId,
  viewer: UserId,
  base: &str,
  now: i64,
) -> Result<ManagedProject, ServerError> {
  let id = project.to_string();
  let (members, open_invites, created_at, created_by): (u32, u32, i64, String) = db.query_row(
    "SELECT (SELECT COUNT(*) FROM memberships WHERE project=p.id),
        (SELECT COUNT(*) FROM invites WHERE project=p.id AND accepted_at IS NULL AND revoked_at IS NULL
          AND expires_at>?2),
        p.created_at, p.created_by
      FROM projects p WHERE p.id=?1",
    params![id, now],
    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
  )?;
  Ok(ManagedProject {
    project: projects::load(db, project, viewer, base)?,
    members,
    open_invites,
    owners: owners(db, &id)?,
    created_at,
    created_by: access::user_ref(db, created_by.parse().map_err(|_| ServerError::StoredData)?)?,
  })
}

fn owners(db: &Connection, project: &str) -> Result<Vec<UserRef>, ServerError> {
  let mut query =
    db.prepare("SELECT user_id FROM memberships WHERE project=?1 AND role='owner' ORDER BY added_at,user_id")?;
  let ids = query
    .query_map([project], |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  ids
    .into_iter()
    .map(|id| access::user_ref(db, id.parse().map_err(|_| ServerError::StoredData)?))
    .collect()
}
