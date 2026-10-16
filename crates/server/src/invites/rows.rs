//! Invite rows as stored, and what an invite looks like to whoever manages it: where it stands, and who ended it.

use std::str::FromStr;

use orchester_api::{Invite, InviteDelivery, InviteKind, InviteState, MemberRole};
use orchester_core::id::{InviteId, ProjectId, UserId};
use rusqlite::{Connection, OptionalExtension, Row};

use crate::{error::ServerError, sharing::access};

pub(super) const COLUMNS: &str = "id,kind,delivery,email,project,role,user_id,admin,made_by,expires_at,created_at,accepted_at,accepted_by,revoked_at,revoked_by";

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
  pub(super) expires_at: i64,
  created_at: i64,
  accepted: Option<(i64, Option<UserId>)>,
  revoked: Option<(i64, Option<UserId>)>,
}

impl Stored {
  /// The invite as it stands at `now`.
  pub(super) fn invite(self, db: &Connection, now: i64) -> Result<Invite, ServerError> {
    let (state, ended_at, ended_by) = match (self.accepted, self.revoked) {
      (Some((at, by)), _) => (InviteState::Accepted, Some(at), by),
      (None, Some((at, by))) => (InviteState::Revoked, Some(at), by),
      (None, None) if self.expires_at <= now => (InviteState::Expired, Some(self.expires_at), None),
      (None, None) => (InviteState::Open, None, None),
    };
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
      created_at: self.created_at,
      state,
      ended_at,
      ended_by: ended_by.map(|user| access::user_ref(db, user)).transpose()?,
    })
  }
}

pub(super) fn load(db: &Connection, id: InviteId, now: i64) -> Result<Option<Invite>, ServerError> {
  db.query_row(
    &format!("SELECT {COLUMNS} FROM invites WHERE id=?1"),
    [id.to_string()],
    stored,
  )
  .optional()?
  .map(|row| row.invite(db, now))
  .transpose()
}

pub(super) fn stored(row: &Row<'_>) -> rusqlite::Result<Stored> {
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
    created_at: row.get(10)?,
    accepted: row
      .get::<_, Option<i64>>(11)?
      .map(|at| Ok::<_, rusqlite::Error>((at, parsed(row, 12)?)))
      .transpose()?,
    revoked: row
      .get::<_, Option<i64>>(13)?
      .map(|at| Ok::<_, rusqlite::Error>((at, parsed(row, 14)?)))
      .transpose()?,
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

pub(super) const fn kind_name(kind: InviteKind) -> &'static str {
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

pub(super) const fn delivery_name(delivery: InviteDelivery) -> &'static str {
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
