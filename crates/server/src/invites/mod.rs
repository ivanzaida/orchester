//! Invites (`docs/architecture/server.md`, "Invites"): to the server, to one project with a role, or for an existing
//! account, delivered by email through Orchester Cloud or as a code. An invite's email is kept only while it is open.

mod repeated;
mod rows;
mod signed;

use orchester_api::{
  ErrorCode, Invite, InviteCreate, InviteCreated, InviteDelivery, InviteKind, InviteRevoked, JoinedProject, MemberRole,
  User, UserChanges,
};
use orchester_core::id::{InviteId, ProjectId, UserId};
pub(crate) use rows::Stored;
use rows::{COLUMNS, delivery_name, kind_name, load, stored};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{
  codes::{self, CodeKind, NewCode},
  db::{self, Authenticated},
  error::ServerError,
  server_keys::ServerKeys,
  sharing::{access, members},
};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// An invite by email is open for 7 days.
const CLOUD_LIFETIME_MS: i64 = 7 * DAY_MS;
/// Invites by email one person may send in 24 hours: each one is an email the cloud sends to someone.
const EMAILS_PER_DAY: i64 = 30;
/// How long ended invites stay in the server admin's *Open and recent invites*.
const RECENT_MS: i64 = 30 * DAY_MS;

/// Which invites a list shows: one project's or all the viewer manages, and the recently ended ones too.
pub(crate) struct Listing {
  pub project: Option<ProjectId>,
  pub recent: bool,
}

/// A new invite by `actor`'s device token, by email or with a code: it needs a token issued for a cloud identity that
/// signed in with two-factor authentication, or a browser signed in with a pairing code such a token made
/// (`403 mfa_required`), checked after who may make it. Invites already made are delivered as before. An invite by
/// email that repeats an open one, or would give its address nothing, is refused (`repeated`).
pub(crate) fn create(
  db: &Connection,
  keys: &ServerKeys,
  actor: &Authenticated,
  input: &InviteCreate,
  base_address: &str,
  now: i64,
) -> Result<InviteCreated, ServerError> {
  let (session, actor) = (actor, &actor.user);
  check(db, actor, input)?;
  if !session.mfa {
    return Err(ErrorCode::MfaRequired.into());
  }
  let email = input.email.as_deref().map(str::to_ascii_lowercase);
  if let Some(email) = &email {
    repeated::refuse(db, keys, input, email, now)?;
  }
  if input.delivery == InviteDelivery::Cloud {
    daily_limit(db, actor.id, now)?;
  }
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
  db.execute(
    "INSERT INTO invites(id,kind,delivery,email,project,role,user_id,admin,made_by,expires_at,created_at)
      VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
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
  let invite = load(db, id, now)?.ok_or(ServerError::StoredData)?;
  let (code, signed) = match input.delivery {
    InviteDelivery::Code => {
      let new = NewCode {
        kind: code_kind,
        invite: Some(id),
        user: input.user,
        made_by: Some(actor.id),
        mfa: false,
      };
      (Some(codes::create(db, keys, &new, now)?.0), None)
    }
    InviteDelivery::Cloud => (None, Some(signed::invite(db, keys, &invite, base_address, now)?)),
  };
  Ok(InviteCreated { invite, code, signed })
}

/// `429` once `inviter` sent [`EMAILS_PER_DAY`] invites by email in the last 24 hours, until the oldest of them is a
/// day old.
fn daily_limit(db: &Connection, inviter: UserId, now: i64) -> Result<(), ServerError> {
  let (sent, oldest): (i64, Option<i64>) = db.query_row(
    "SELECT COUNT(*),MIN(created_at) FROM invites WHERE made_by=?1 AND delivery='cloud' AND created_at>?2",
    params![inviter.to_string(), now - DAY_MS],
    |row| Ok((row.get(0)?, row.get(1)?)),
  )?;
  match oldest {
    Some(oldest) if sent >= EMAILS_PER_DAY => {
      let wait = (oldest + DAY_MS - now).max(1000);
      Err(ServerError::RateLimited(
        u64::try_from(wait.div_euclid(1000)).unwrap_or(1),
      ))
    }
    _ => Ok(()),
  }
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
    _ if actor.admin => match input.user.map(|user| db::user_by_id(db, user)).transpose()? {
      Some(None) => Err(ErrorCode::NoSuchUser.into()),
      // An admin's own next device comes from their own devices (a pairing code) or the host, never from another
      // admin: an account invite would let one admin sign in as another.
      Some(Some(user)) if user.admin => Err(ErrorCode::Forbidden.into()),
      _ => Ok(()),
    },
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

/// The invites `viewer` manages: every one for an admin, their projects' for an owner; the open ones, and with
/// `recent` those that ended in the last 30 days, newest first. Never a code; an ended invite has no email.
pub(crate) fn list(db: &Connection, viewer: &User, listing: &Listing, now: i64) -> Result<Vec<Invite>, ServerError> {
  forget_ended(db, now)?;
  if let Some(project) = listing.project {
    project_owner(db, viewer, project)?;
  }
  let mut query = db.prepare(&format!(
    "SELECT {COLUMNS} FROM invites
      WHERE (?2 OR project IN (SELECT project FROM memberships WHERE user_id=?3 AND role='owner'))
        AND (?4 IS NULL OR project=?4)
        AND ((accepted_at IS NULL AND revoked_at IS NULL AND expires_at>?1)
          OR (?5 AND COALESCE(accepted_at,revoked_at,expires_at)>?1-?6))
      ORDER BY created_at DESC,id"
  ))?;
  let rows = query
    .query_map(
      params![
        now,
        viewer.admin,
        viewer.id.to_string(),
        listing.project.map(|project| project.to_string()),
        listing.recent,
        RECENT_MS
      ],
      stored,
    )?
    .collect::<Result<Vec<_>, _>>()?;
  rows.into_iter().map(|row| row.invite(db, now)).collect()
}

/// Ends an open invite; its code stops working. For an invite by email, the answer carries the revocation the
/// server signed for the cloud.
pub(crate) fn revoke(
  db: &Connection,
  keys: &ServerKeys,
  viewer: &User,
  id: InviteId,
  now: i64,
) -> Result<InviteRevoked, ServerError> {
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
    "UPDATE invites SET revoked_at=?1, revoked_by=?2, email=NULL WHERE id=?3",
    params![now, viewer.id.to_string(), id.to_string()],
  )?;
  codes::end_invite(db, id)?;
  db::audit(db, Some(viewer.id), "invite_revoked", &id.to_string(), None, now)?;
  let revocation = (invite.delivery == InviteDelivery::Cloud)
    .then(|| signed::revocation(keys, id, now))
    .transpose()?;
  Ok(InviteRevoked { revocation })
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

/// Whether `invite` may still be accepted, checked again at acceptance: an account invite only while its account is
/// not an admin, an admin invite only while its inviter is an enabled admin. Otherwise `403 forbidden`, and the invite
/// stays open.
pub(crate) fn still_allowed(db: &Connection, invite: &Stored) -> Result<(), ServerError> {
  if invite.kind == InviteKind::Account {
    let target = invite.user.map(|user| db::user_by_id(db, user)).transpose()?.flatten();
    if target.is_some_and(|user| user.admin) {
      return Err(ErrorCode::Forbidden.into());
    }
  }
  if invite.admin {
    let inviter = db::user_by_id(db, invite.made_by)?;
    if !inviter.is_some_and(|inviter| inviter.admin && !inviter.disabled) {
      return Err(ErrorCode::Forbidden.into());
    }
  }
  Ok(())
}

/// Ends `invite` as accepted by `user` and gives what it grants: the project membership, unless `user` has it already.
/// An invite that is no longer allowed ([`still_allowed`]) is refused.
pub(crate) fn accept(
  db: &Connection,
  invite: &Stored,
  user: UserId,
  now: i64,
) -> Result<Option<JoinedProject>, ServerError> {
  still_allowed(db, invite)?;
  db.execute(
    "UPDATE invites SET accepted_at=?1, accepted_by=?2, email=NULL WHERE id=?3",
    params![now, user.to_string(), invite.id.to_string()],
  )?;
  db::audit(db, Some(user), "invite_accepted", &invite.id.to_string(), None, now)?;
  if invite.admin {
    // An admin invite accepted by an account the server knows makes it an admin, as one made by the invite would be.
    let changes = UserChanges {
      display_name: None,
      admin: Some(true),
      disabled: None,
    };
    db::patch_user(db, user, &changes, invite.made_by, now)?;
  }
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

fn valid_email(email: &str) -> bool {
  let parts = email.split_once('@');
  email.len() <= 254
    && !email.chars().any(char::is_whitespace)
    && parts.is_some_and(|(local, domain)| !local.is_empty() && domain.contains('.') && !domain.contains('@'))
}
