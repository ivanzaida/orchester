//! An invite by email is not made twice (`docs/architecture/server.md`, "Invites"): not while its address has an open
//! one for the same thing, and not to someone the invite would give nothing. Checked in the transaction that makes the
//! invite, which the server's one writer runs at a time, so two quick submits cannot both pass.

use orchester_api::{ErrorCode, InviteCreate, SharingErrorDetails};
use orchester_core::id::InviteId;
use rusqlite::{Connection, OptionalExtension, params};

use super::rows::kind_name;
use crate::{error::ServerError, server_keys::ServerKeys, sharing};

/// `409 invite_open` naming the open invite to `email` of the same kind, project and account, or `409 already_member`
/// when the account with that email (as its cloud identity last verified it) is a member of the project already, or,
/// for an invite to the server, has an account here that the invite would not make an admin.
pub(super) fn refuse(
  db: &Connection,
  keys: &ServerKeys,
  input: &InviteCreate,
  email: &str,
  now: i64,
) -> Result<(), ServerError> {
  let project = input.project.map(|project| project.to_string());
  let open: Option<(String, i64)> = db
    .query_row(
      "SELECT id,created_at FROM invites WHERE email=?1 AND kind=?2 AND project IS ?3 AND user_id IS ?4
        AND accepted_at IS NULL AND revoked_at IS NULL AND expires_at>?5 ORDER BY created_at,id LIMIT 1",
      params![
        email,
        kind_name(input.kind),
        project,
        input.user.map(|user| user.to_string()),
        now
      ],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?;
  if let Some((id, sent_at)) = open {
    let invite: InviteId = id.parse().map_err(|_| ServerError::StoredData)?;
    return Err(sharing::conflict(
      ErrorCode::InviteOpen,
      SharingErrorDetails::Invite { invite, sent_at },
    ));
  }
  if gives_nothing(db, &keys.email_tag(email), input)? {
    return Err(ErrorCode::AlreadyMember.into());
  }
  Ok(())
}

/// Whether the account `tag` names already has what the invite gives: the project's membership, or an account on the
/// server (an admin's, for an invite as an admin). An account invite is for an account that exists.
fn gives_nothing(db: &Connection, tag: &[u8], input: &InviteCreate) -> Result<bool, ServerError> {
  Ok(db.query_row(
    "SELECT EXISTS(SELECT 1 FROM users u WHERE u.email_tag=?1 AND CASE ?2
      WHEN 'project' THEN EXISTS(SELECT 1 FROM memberships m WHERE m.user_id=u.id AND m.project=?3)
      WHEN 'server' THEN u.admin OR NOT ?4
      ELSE 0 END)",
    params![
      tag,
      kind_name(input.kind),
      input.project.map(|project| project.to_string()),
      input.admin
    ],
    |row| row.get(0),
  )?)
}
