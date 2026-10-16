//! Invites this computer accepted through Orchester Cloud and has not finished joining (N1 security review 1, M2): the
//! cloud lists an accepted invite no more, so this computer keeps what it needs to ask for another token for it, the
//! team server it is from, and the approval it waits for when join approval is on. Gone once joined, refused, ended,
//! expired or set aside by the person, and with the cloud account it belongs to.

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
}

impl AppDb {
  /// The invites accepted here that have not expired by `now`, oldest first; expired ones are forgotten.
  pub fn accepted_invites(&mut self, now: Timestamp) -> Result<Vec<AcceptedInvite>, StoreError> {
    self.connection.execute(
      "DELETE FROM cloud_accepted_invites WHERE expires_at <= ?1",
      [now.unix_millis()],
    )?;
    let mut query = self.connection.prepare(
      "SELECT invite, team_server, server_name, project_name, expires_at, approval, accepted_at \
       FROM cloud_accepted_invites ORDER BY accepted_at, invite",
    )?;
    let rows = query.query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, Option<String>>(3)?,
        row.get::<_, i64>(4)?,
        row.get::<_, Option<String>>(5)?,
        row.get::<_, i64>(6)?,
      ))
    })?;
    rows
      .map(|row| {
        let (invite, server, server_name, project_name, expires_at, approval, accepted_at) = row?;
        Ok(AcceptedInvite {
          invite: parse("invite id", &invite)?,
          server: parse("team server id", &server)?,
          server_name,
          project_name,
          expires_at: Timestamp::from_unix_millis(expires_at),
          approval: approval.map(|approval| parse("approval id", &approval)).transpose()?,
          accepted_at: Timestamp::from_unix_millis(accepted_at),
        })
      })
      .collect()
  }

  /// Keeps `invite`, replacing what was kept for it (the approval it now waits for).
  pub fn keep_accepted_invite(&mut self, invite: &AcceptedInvite) -> Result<(), StoreError> {
    self.connection.execute(
      "INSERT INTO cloud_accepted_invites \
         (invite, team_server, server_name, project_name, expires_at, approval, accepted_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
       ON CONFLICT (invite) DO UPDATE SET approval = excluded.approval",
      params![
        invite.invite.to_string(),
        invite.server.to_string(),
        invite.server_name,
        invite.project_name,
        invite.expires_at.unix_millis(),
        invite.approval.map(|approval| approval.to_string()),
        invite.accepted_at.unix_millis(),
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
