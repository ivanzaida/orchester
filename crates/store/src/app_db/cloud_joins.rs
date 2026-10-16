//! Invites this computer accepted through Orchester Cloud and has not finished joining (N1 security review 1, M2): the
//! cloud lists an accepted invite no more, so this computer keeps what it needs to ask for another token for it, the
//! team server it is from, and the approval it waits for when join approval is on. Each ask takes an identity token,
//! so it also keeps how often it asked and when it asks again on its own (N1 security re-review 2). Gone once joined,
//! refused, ended, expired or set aside by the person, and with the cloud account it belongs to.

use orchester_core::{
  id::{ApprovalId, InviteId, TeamServerId},
  time::Timestamp,
};
use rusqlite::params;

use super::AppDb;
use crate::{StoreError, rows::parse};

pub(super) const MIGRATION: &str = r"
CREATE TABLE cloud_accepted_invites (
  invite TEXT PRIMARY KEY,
  team_server TEXT NOT NULL,
  server_name TEXT NOT NULL,
  project_name TEXT,
  expires_at INTEGER NOT NULL,
  approval TEXT,
  accepted_at INTEGER NOT NULL
) STRICT;
";

/// How often an accepted invite was asked for, and when it is asked again (N1 security re-review 2).
pub(super) const ASKS_MIGRATION: &str = r"
ALTER TABLE cloud_accepted_invites ADD COLUMN approval_expires_at INTEGER;
ALTER TABLE cloud_accepted_invites ADD COLUMN asks INTEGER NOT NULL DEFAULT 0;
ALTER TABLE cloud_accepted_invites ADD COLUMN checks INTEGER NOT NULL DEFAULT 0;
ALTER TABLE cloud_accepted_invites ADD COLUMN next_check_at INTEGER;
";

const COLUMNS: &str = "invite, team_server, server_name, project_name, expires_at, approval, accepted_at, \
  approval_expires_at, asks, checks, next_check_at";

/// An invite accepted here whose join has not finished.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcceptedInvite {
  pub invite: InviteId,
  /// The team server the invite is from, by its id at the cloud.
  pub server: TeamServerId,
  pub server_name: String,
  /// The project the invite is to, by its name when the invite was made.
  pub project_name: Option<String>,
  pub expires_at: Timestamp,
  /// The team server holds the join until this approval is given.
  pub approval: Option<ApprovalId>,
  pub accepted_at: Timestamp,
  /// When that approval expires: it is not asked for after that.
  pub approval_expires_at: Option<Timestamp>,
  /// How often this computer asked the team server for the invite, each time with a new identity token. Never goes
  /// down.
  pub asks: u32,
  /// How often it was asked again on its own while waiting for the approval, since the person last asked.
  pub checks: u32,
  /// When it is asked again on its own; `None`: only when the person asks.
  pub next_check_at: Option<Timestamp>,
}

impl AcceptedInvite {
  /// What is kept of an invite accepted at `accepted_at`, before anything was asked for it.
  pub fn new(
    (invite, server, server_name): (InviteId, TeamServerId, String),
    project_name: Option<String>,
    expires_at: Timestamp,
    accepted_at: Timestamp,
  ) -> Self {
    Self {
      invite,
      server,
      server_name,
      project_name,
      expires_at,
      approval: None,
      accepted_at,
      approval_expires_at: None,
      asks: 0,
      checks: 0,
      next_check_at: None,
    }
  }
}

/// One row as SQLite holds it.
type Row = (
  (String, String, String, Option<String>),
  (i64, Option<String>, i64),
  (Option<i64>, u32, u32, Option<i64>),
);

impl AppDb {
  /// The invites accepted here that have not expired by `now`, oldest first; expired ones are forgotten.
  pub fn accepted_invites(&mut self, now: Timestamp) -> Result<Vec<AcceptedInvite>, StoreError> {
    self.connection.execute(
      "DELETE FROM cloud_accepted_invites WHERE expires_at <= ?1",
      [now.unix_millis()],
    )?;
    let mut query = self.connection.prepare(&format!(
      "SELECT {COLUMNS} FROM cloud_accepted_invites ORDER BY accepted_at, invite"
    ))?;
    let rows = query.query_map([], |row| -> rusqlite::Result<Row> {
      Ok((
        (row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?),
        (row.get(4)?, row.get(5)?, row.get(6)?),
        (row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?),
      ))
    })?;
    rows.map(|row| accepted(row?)).collect()
  }

  /// Keeps `invite`, replacing what was kept for it (the approval it now waits for).
  pub fn keep_accepted_invite(&mut self, invite: &AcceptedInvite) -> Result<(), StoreError> {
    self.connection.execute(
      &format!(
        "INSERT INTO cloud_accepted_invites ({COLUMNS}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
         ON CONFLICT (invite) DO UPDATE SET approval = excluded.approval, \
           approval_expires_at = excluded.approval_expires_at, asks = max(asks, excluded.asks), \
           checks = excluded.checks, next_check_at = excluded.next_check_at"
      ),
      params![
        invite.invite.to_string(),
        invite.server.to_string(),
        invite.server_name,
        invite.project_name,
        invite.expires_at.unix_millis(),
        invite.approval.map(|approval| approval.to_string()),
        invite.accepted_at.unix_millis(),
        invite.approval_expires_at.map(Timestamp::unix_millis),
        invite.asks,
        invite.checks,
        invite.next_check_at.map(Timestamp::unix_millis),
      ],
    )?;
    Ok(())
  }

  /// Forgets `invite`: joined, refused, ended or set aside.
  pub fn forget_accepted_invite(&mut self, invite: InviteId) -> Result<(), StoreError> {
    self.connection.execute(
      "DELETE FROM cloud_accepted_invites WHERE invite = ?1",
      [invite.to_string()],
    )?;
    Ok(())
  }

  /// Forgets every accepted invite, with the cloud account they belong to.
  pub fn forget_accepted_invites(&mut self) -> Result<(), StoreError> {
    self.connection.execute("DELETE FROM cloud_accepted_invites", [])?;
    Ok(())
  }
}

fn accepted(
  (
    (invite, server, server_name, project_name),
    (expires_at, approval, accepted_at),
    (approval_expires_at, asks, checks, next_check_at),
  ): Row,
) -> Result<AcceptedInvite, StoreError> {
  Ok(AcceptedInvite {
    invite: parse("invite id", &invite)?,
    server: parse("team server id", &server)?,
    server_name,
    project_name,
    expires_at: Timestamp::from_unix_millis(expires_at),
    approval: approval.map(|approval| parse("approval id", &approval)).transpose()?,
    accepted_at: Timestamp::from_unix_millis(accepted_at),
    approval_expires_at: approval_expires_at.map(Timestamp::from_unix_millis),
    asks,
    checks,
    next_check_at: next_check_at.map(Timestamp::from_unix_millis),
  })
}
