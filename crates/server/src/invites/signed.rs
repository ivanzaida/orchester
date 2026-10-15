//! What the server signs for Orchester Cloud about its invites by email: the invite for the cloud to route, and its
//! revocation. The server admin's page delivers both; the server itself needs no internet.

use orchester_api::{
  Invite,
  statements::{INVITE_REVOCATION_TYPE, InviteRevocation, InvitedProject, SIGNED_INVITE_TYPE, SignedInvite},
};
use orchester_core::id::InviteId;
use rusqlite::Connection;

use crate::{error::ServerError, server_keys::ServerKeys};

pub(super) fn invite(
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
          Ok(InvitedProject {
            address: format!("{base_address}/{}", row.get::<_, String>(0)?),
            name: row.get(1)?,
          })
        },
      )
    })
    .transpose()?;
  let statement = SignedInvite {
    id: invite.id,
    server: keys.id,
    kind: invite.kind,
    email: invite.email.clone().ok_or(ServerError::StoredData)?,
    project,
    role: invite.role,
    inviter: invite.by.display_name.clone(),
    expires_at: invite.expires_at,
    issued_at: now,
  };
  keys.sign(SIGNED_INVITE_TYPE, &statement)
}

pub(super) fn revocation(keys: &ServerKeys, id: InviteId, now: i64) -> Result<String, ServerError> {
  keys.sign(
    INVITE_REVOCATION_TYPE,
    &InviteRevocation {
      id,
      server: keys.id,
      issued_at: now,
    },
  )
}
