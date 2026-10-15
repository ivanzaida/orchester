//! Invites (`docs/architecture/server.md`, "Invites"): to the server, to one project with a role, or for an existing
//! account, delivered by email through Orchester Cloud or as a code. An invite's email is kept only while it is open.

use std::str::FromStr;

use orchester_api::{
  ErrorCode, Invite, InviteCreate, InviteCreated, InviteDelivery, InviteKind, JoinedProject, MemberRole, User,
};
use orchester_core::id::{InviteId, ProjectId, UserId};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::Serialize;

use crate::{
  codes::{self, CodeKind, NewCode},
  db,
  error::ServerError,
  server_keys::ServerKeys,
  sharing::{access, members},
};

/// An invite by email is open for 7 days.
const CLOUD_LIFETIME_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const COLUMNS: &str = "id,kind,delivery,email,project,role,user_id,admin,made_by,expires_at";

/// What the server signs for the cloud to route an invite; the server admin's page carries it when the server cannot
/// reach the cloud.
#[derive(Serialize)]
struct SignedInvite<'a> {
  id: InviteId,
  server: String,
  kind: InviteKind,
  email: &'a str,
  project: Option<SignedProject>,
  role: Option<MemberRole>,
  inviter: &'a str,
  expires_at: i64,
  issued_at: i64,
}

#[derive(Serialize)]
struct SignedProject {
  address: String,
  name: String,
}

pub(crate) fn create(
  db: &Connection,
  keys: &ServerKeys,
  actor: &User,
  input: &InviteCreate,
  base_address: &str,
  now: i64,
) -> Result<InviteCreated, ServerError> {
  check(db, actor, input)?;
  let id = InviteId::generate();
  let code_kind = if input.kind == InviteKind::Account {
    CodeKind::Account
  } else {
    CodeKind::Invite
  };
  let expires_at = match input.delivery {
    InviteDelivery::Cloud => now + CLOUD_LIFETIME_MS,
    InviteDelivery::Code => now + code_kind.lifetime_ms(),
  };
  let email = input.email.as_deref().map(str::to_ascii_lowercase);
  db.execute(
    &format!("INSERT INTO invites({COLUMNS},created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)"),
    params![
      id.to_string(),
      kind_name(input.kind),
      delivery_name(input.delivery),
      email,
      input.project.map(|project| project.to_string()),
      input.role.map(access::role_name),
      input.user.map(|user| user.to_string()),
      input.admin,
      actor.id.to_string(),
      expires_at,
      now
    ],
  )?;
  db::audit(db, Some(actor.id), "invite_made", &id.to_string(), None, now)?;
  let invite = load(db, id)?.ok_or(ServerError::StoredData)?;
  let (code, signed) = match input.delivery {
    InviteDelivery::Code => {
      let new = NewCode {
        kind: code_kind,
        invite: Some(id),
        user: input.user,
        made_by: Some(actor.id),
      };
      (Some(codes::create(db, keys, &new, now)?.0), None)
    }
    InviteDelivery::Cloud => (None, Some(sign(db, keys, &invite, base_address, now)?)),
  };
  Ok(InviteCreated { invite, code, signed })
}

/// Who may make which invite, and the fields each kind needs.
fn check(db: &Connection, actor: &User, input: &InviteCreate) -> Result<(), ServerError> {
  let email_ok = match (&input.email, input.delivery) {
    (Some(email), InviteDelivery::Cloud) => valid_email(email),
    (None, InviteDelivery::Code) => true,
    _ => false,
  };
  let shape_ok = match input.kind {
    InviteKind::Server => input.project.is_none() && input.role.is_none() && input.user.is_none(),
    InviteKind::Project => input.project.is_some() && input.role.is_some() && input.user.is_none() && !input.admin,
    InviteKind::Account => input.project.is_none() && input.role.is_none() && input.user.is_some() && !input.admin,
  };
  if !email_ok || !shape_ok {
    return Err(ErrorCode::Invalid.into());
  }
  match (input.kind, input.project) {
    (InviteKind::Project, Some(project)) => project_owner(db, actor, project),
    _ if actor.admin => {
      let user = input.user.map(|user| db::user_by_id(db, user)).transpose()?;
      if user.is_some_and(|user| user.is_none()) {
        return Err(ErrorCode::NoSuchUser.into());
      }
      Ok(())
    }
    _ => Err(ErrorCode::Forbidden.into()),
  }
}

/// An admin, or an owner of `project`; a project the actor cannot see answers `404`.
fn project_owner(db: &Connection, actor: &User, project: ProjectId) -> Result<(), ServerError> {
  let exists: bool = db.query_row(
    "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
    [project.to_string()],
    |row| row.get(0),
  )?;
  if !exists {
    return Err(ErrorCode::NotFound.into());
  }
  if actor.admin {
    return Ok(());
  }
  match access::role(db, project, actor.id)? {
    Some(MemberRole::Owner) => Ok(()),
    Some(MemberRole::Member) => Err(ErrorCode::Forbidden.into()),
    None => Err(ErrorCode::NotFound.into()),
  }
}

fn sign(
  db: &Connection,
  keys: &ServerKeys,
  invite: &Invite,
  base_address: &str,
  now: i64,
) -> Result<String, ServerError> {
  let project = invite
    .project
    .map(|project| {
      db.query_row(
        "SELECT slug,name FROM projects WHERE id=?1",
        [project.to_string()],
        |row| {
          Ok(SignedProject {
            address: format!("{base_address}/{}", row.get::<_, String>(0)?),
            name: row.get(1)?,
          })
        },
      )
    })
    .transpose()?;
  let statement = SignedInvite {
    id: invite.id,
    server: keys.id.to_string(),
    kind: invite.kind,
    email: invite.email.as_deref().ok_or(ServerError::StoredData)?,
    project,
    role: invite.role,
    inviter: &invite.by.display_name,
    expires_at: invite.expires_at,
    issued_at: now,
  };
  Ok(keys.sign("orchester-invite+jws", &statement))
}

/// The open invites `viewer` manages: every one for an admin, their projects' for an owner. Never a code.
pub(crate) fn list(db: &Connection, viewer: &User, now: i64) -> Result<Vec<Invite>, ServerError> {
  forget_ended(db, now)?;
  let mut query = db.prepare(&format!(
    "SELECT {COLUMNS} FROM invites WHERE accepted_at IS NULL AND revoked_at IS NULL AND expires_at>?1
      AND (?2 OR project IN (SELECT project FROM memberships WHERE user_id=?3 AND role='owner'))
      ORDER BY created_at,id"
  ))?;
  let rows = query
    .query_map(params![now, viewer.admin, viewer.id.to_string()], stored)?
    .collect::<Result<Vec<_>, _>>()?;
  rows.into_iter().map(|row| row.invite(db)).collect()
}

/// Ends an open invite; its code stops working.
pub(crate) fn revoke(db: &Connection, viewer: &User, id: InviteId, now: i64) -> Result<(), ServerError> {
  let invite = open(db, id, now)?.ok_or(ErrorCode::NotFound)?;
  let allowed = viewer.admin
    || invite.made_by == viewer.id
    || invite
      .project
      .map(|project| access::role(db, project, viewer.id))
      .transpose()?
      .flatten()
      == Some(MemberRole::Owner);
  if !allowed {
    return Err(ErrorCode::NotFound.into());
  }
  db.execute(
    "UPDATE invites SET revoked_at=?1, email=NULL WHERE id=?2",
    params![now, id.to_string()],
  )?;
  codes::end_invite(db, id)?;
  db::audit(db, Some(viewer.id), "invite_revoked", &id.to_string(), None, now)
}

/// An invite still open at `now`, as stored.
pub(crate) fn open(db: &Connection, id: InviteId, now: i64) -> Result<Option<Stored>, ServerError> {
  Ok(
    db.query_row(
      &format!(
        "SELECT {COLUMNS} FROM invites WHERE id=?1 AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at>?2"
      ),
      params![id.to_string(), now],
      stored,
    )
    .optional()?,
  )
}

/// Ends `invite` as accepted by `user` and gives what it grants: the project membership, unless `user` has it already.
pub(crate) fn accept(
  db: &Connection,
  invite: &Stored,
  user: UserId,
  now: i64,
) -> Result<Option<JoinedProject>, ServerError> {
  db.execute(
    "UPDATE invites SET accepted_at=?1, accepted_by=?2, email=NULL WHERE id=?3",
    params![now, user.to_string(), invite.id.to_string()],
  )?;
  db::audit(db, Some(user), "invite_accepted", &invite.id.to_string(), None, now)?;
  let (Some(project), Some(role)) = (invite.project, invite.role) else {
    return Ok(None);
  };
  let role = match access::role(db, project, user)? {
    Some(current) => current,
    None => members::insert(db, project, user, role, invite.made_by, now)?.role,
  };
  Ok(Some(JoinedProject { project, role }))
}

fn forget_ended(db: &Connection, now: i64) -> Result<(), ServerError> {
  db.execute(
    "UPDATE invites SET email=NULL WHERE email IS NOT NULL AND expires_at<=?1",
    [now],
  )?;
  Ok(())
}

/// An invite row.
pub(crate) struct Stored {
  pub id: InviteId,
  pub kind: InviteKind,
  pub delivery: InviteDelivery,
  pub email: Option<String>,
  pub project: Option<ProjectId>,
  pub role: Option<MemberRole>,
  pub user: Option<UserId>,
  pub admin: bool,
  pub made_by: UserId,
  expires_at: i64,
}

impl Stored {
  fn invite(self, db: &Connection) -> Result<Invite, ServerError> {
    Ok(Invite {
      id: self.id,
      kind: self.kind,
      email: self.email,
      project: self.project,
      role: self.role,
      user: self.user.map(|user| access::user_ref(db, user)).transpose()?,
      admin: self.admin,
      by: access::user_ref(db, self.made_by)?,
      delivery: self.delivery,
      expires_at: self.expires_at,
    })
  }
}

fn load(db: &Connection, id: InviteId) -> Result<Option<Invite>, ServerError> {
  db.query_row(
    &format!("SELECT {COLUMNS} FROM invites WHERE id=?1"),
    [id.to_string()],
    stored,
  )
  .optional()?
  .map(|row| row.invite(db))
  .transpose()
}

fn stored(row: &Row<'_>) -> rusqlite::Result<Stored> {
  Ok(Stored {
    id: required(row, 0)?,
    kind: parse_kind(&row.get::<_, String>(1)?).ok_or_else(|| conversion(1))?,
    delivery: parse_delivery(&row.get::<_, String>(2)?).ok_or_else(|| conversion(2))?,
    email: row.get(3)?,
    project: parsed(row, 4)?,
    role: row
      .get::<_, Option<String>>(5)?
      .map(|role| access::parse_role(&role).ok_or_else(|| conversion(5)))
      .transpose()?,
    user: parsed(row, 6)?,
    admin: row.get(7)?,
    made_by: required(row, 8)?,
    expires_at: row.get(9)?,
  })
}

fn parsed<T: FromStr>(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<T>> {
  row
    .get::<_, Option<String>>(index)?
    .map(|value| value.parse().map_err(|_| conversion(index)))
    .transpose()
}

fn required<T: FromStr>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
  parsed(row, index)?.ok_or_else(|| conversion(index))
}

fn conversion(index: usize) -> rusqlite::Error {
  rusqlite::Error::InvalidColumnType(index, "invite".into(), rusqlite::types::Type::Text)
}

const fn kind_name(kind: InviteKind) -> &'static str {
  match kind {
    InviteKind::Server => "server",
    InviteKind::Project => "project",
    InviteKind::Account => "account",
  }
}

fn parse_kind(value: &str) -> Option<InviteKind> {
  [InviteKind::Server, InviteKind::Project, InviteKind::Account]
    .into_iter()
    .find(|kind| kind_name(*kind) == value)
}

const fn delivery_name(delivery: InviteDelivery) -> &'static str {
  match delivery {
    InviteDelivery::Cloud => "cloud",
    InviteDelivery::Code => "code",
  }
}

fn parse_delivery(value: &str) -> Option<InviteDelivery> {
  [InviteDelivery::Cloud, InviteDelivery::Code]
    .into_iter()
    .find(|delivery| delivery_name(*delivery) == value)
}

fn valid_email(email: &str) -> bool {
  let parts = email.split_once('@');
  email.len() <= 254
    && !email.chars().any(char::is_whitespace)
    && parts.is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.') && !domain.contains('@'))
}
