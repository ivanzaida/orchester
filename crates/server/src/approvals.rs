//! Join approval (`docs/architecture/server.md`, row 57; "The cloud's trust boundary"): while it is on, a cloud join
//! of an existing account waits until one of that account's joined devices or an admin approves it. The joining app
//! asks again with the approval's id; a refused approval ends the join.

use std::net::IpAddr;

use orchester_api::{Approval, ApprovalState, ErrorCode, User};
use orchester_core::id::{ApprovalId, UserId};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::{db, error::ServerError, sharing::access};

/// How long a join waits for approval.
const LIFETIME_MS: i64 = 60 * 60 * 1000;
const COLUMNS: &str = "id,user_id,device,state,requested_at,expires_at";

pub(crate) fn enabled(db: &Connection) -> Result<bool, ServerError> {
  Ok(db.query_row("SELECT join_approval FROM settings WHERE id=1", [], |row| row.get(0))?)
}

pub(crate) fn set_enabled(db: &Connection, enabled: bool, actor: UserId, now: i64) -> Result<(), ServerError> {
  db.execute("UPDATE settings SET join_approval=?1 WHERE id=1", [enabled])?;
  db::audit(
    db,
    Some(actor),
    "join_approval_changed",
    if enabled { "on" } else { "off" },
    None,
    now,
  )
}

/// The approval `user`'s cloud join as `subject` waits for: the one already waiting, or a new one for `device` at
/// `address`; `true` when it is new. An account and cloud identity have one waiting approval at a time.
pub(crate) fn request(
  db: &Connection,
  user: UserId,
  subject: &str,
  device: &str,
  address: IpAddr,
  now: i64,
) -> Result<(Approval, bool), ServerError> {
  prune(db, now)?;
  let waiting: Option<String> = db
    .query_row(
      "SELECT id FROM approvals WHERE user_id=?1 AND cloud_subject=?2 AND state='waiting'",
      params![user.to_string(), subject],
      |row| row.get(0),
    )
    .optional()?;
  if let Some(id) = waiting {
    let id = id.parse().map_err(|_| ServerError::StoredData)?;
    return Ok((load(db, id)?.ok_or(ServerError::StoredData)?, false));
  }
  let id = ApprovalId::generate();
  db.execute(
    "INSERT INTO approvals(id,user_id,cloud_subject,device,address,state,requested_at,expires_at)
      VALUES (?1,?2,?3,?4,?5,'waiting',?6,?7)",
    params![
      id.to_string(),
      user.to_string(),
      subject,
      device,
      address.to_string(),
      now,
      now + LIFETIME_MS
    ],
  )?;
  db::audit(
    db,
    Some(user),
    "join_approval_requested",
    &id.to_string(),
    Some(address),
    now,
  )?;
  Ok((load(db, id)?.ok_or(ServerError::StoredData)?, true))
}

/// Removes the approvals that ended: expired, refused or used. A join that names one is refused all the same.
fn prune(db: &Connection, now: i64) -> Result<(), ServerError> {
  db.execute(
    "DELETE FROM approvals WHERE expires_at<=?1 OR state IN ('refused','used')",
    [now],
  )?;
  Ok(())
}

/// What a join that names approval `id` may do: go on once approved (the approval is used up), wait while it waits.
/// Refused, expired, used or another account's: `403 approval_refused`.
pub(crate) fn finish(
  db: &Connection,
  id: ApprovalId,
  user: UserId,
  subject: &str,
  now: i64,
) -> Result<Option<Approval>, ServerError> {
  let row: Option<(String, String, i64)> = db
    .query_row(
      "SELECT user_id,state,expires_at FROM approvals WHERE id=?1 AND cloud_subject=?2",
      params![id.to_string(), subject],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .optional()?;
  match row {
    Some((owner, state, expires_at)) if owner == user.to_string() && expires_at > now => match state.as_str() {
      "approved" => {
        db.execute("UPDATE approvals SET state='used' WHERE id=?1", [id.to_string()])?;
        Ok(None)
      }
      "waiting" => Ok(load(db, id)?),
      _ => Err(ErrorCode::ApprovalRefused.into()),
    },
    _ => Err(ErrorCode::ApprovalRefused.into()),
  }
}

/// The waiting approvals `viewer` may answer: their own account's, or every one for an admin.
pub(crate) fn waiting(db: &Connection, viewer: &User, now: i64) -> Result<Vec<Approval>, ServerError> {
  let mut query = db.prepare(&format!(
    "SELECT {COLUMNS} FROM approvals WHERE state='waiting' AND expires_at>?1 AND (?2 OR user_id=?3) ORDER BY requested_at,id"
  ))?;
  let rows = query
    .query_map(params![now, viewer.admin, viewer.id.to_string()], row)?
    .collect::<Result<Vec<_>, _>>()?;
  rows.into_iter().map(|row| row.approval(db)).collect()
}

/// The answer of `viewer`, one of the account's devices or an admin, to a waiting approval.
pub(crate) fn decide(
  db: &Connection,
  viewer: &User,
  id: ApprovalId,
  approve: bool,
  now: i64,
) -> Result<Approval, ServerError> {
  let changed = db.execute(
    "UPDATE approvals SET state=?1,decided_by=?2,decided_at=?3
      WHERE id=?4 AND state='waiting' AND expires_at>?3 AND (?5 OR user_id=?2)",
    params![
      if approve { "approved" } else { "refused" },
      viewer.id.to_string(),
      now,
      id.to_string(),
      viewer.admin
    ],
  )?;
  if changed == 0 {
    return Err(ErrorCode::NotFound.into());
  }
  let action = if approve { "join_approved" } else { "join_refused" };
  db::audit(db, Some(viewer.id), action, &id.to_string(), None, now)?;
  load(db, id)?.ok_or(ServerError::StoredData)
}

struct Stored {
  id: ApprovalId,
  user: UserId,
  device: String,
  state: ApprovalState,
  requested_at: i64,
  expires_at: i64,
}

impl Stored {
  fn approval(self, db: &Connection) -> Result<Approval, ServerError> {
    Ok(Approval {
      id: self.id,
      user: access::user_ref(db, self.user)?,
      device: self.device,
      state: self.state,
      requested_at: self.requested_at,
      expires_at: self.expires_at,
    })
  }
}

fn load(db: &Connection, id: ApprovalId) -> Result<Option<Approval>, ServerError> {
  db.query_row(
    &format!("SELECT {COLUMNS} FROM approvals WHERE id=?1"),
    [id.to_string()],
    row,
  )
  .optional()?
  .map(|stored| stored.approval(db))
  .transpose()
}

fn row(row: &Row<'_>) -> rusqlite::Result<Stored> {
  let invalid = |index| rusqlite::Error::InvalidColumnType(index, "approval".into(), rusqlite::types::Type::Text);
  let state = match row.get::<_, String>(3)?.as_str() {
    "waiting" => ApprovalState::Waiting,
    "approved" | "used" => ApprovalState::Approved,
    "refused" => ApprovalState::Refused,
    _ => return Err(invalid(3)),
  };
  Ok(Stored {
    id: row.get::<_, String>(0)?.parse().map_err(|_| invalid(0))?,
    user: row.get::<_, String>(1)?.parse().map_err(|_| invalid(1))?,
    device: row.get(2)?,
    state,
    requested_at: row.get(4)?,
    expires_at: row.get(5)?,
  })
}
