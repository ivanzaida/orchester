//! Codes for joining without the internet (`docs/architecture/server.md`, "Codes"): 8 Crockford characters (40
//! bits), shown `ABCD-EFGH`, read without regard to case or the hyphen, one-time and expiring. Only their HMAC is
//! stored; a redemption checks it against each open code in constant time. Codes are shown once, to the person who
//! made them, and never logged, returned again or put in a URL.

use orchester_api::{Credential, ErrorCode, certificate::crockford};
use orchester_core::id::{InviteId, UserId};
use rusqlite::{Connection, params};

use crate::{
  error::ServerError,
  server_keys::{ServerKeys, random},
};

const MINUTE_MS: i64 = 60 * 1000;
const HOUR_MS: i64 = 60 * MINUTE_MS;
/// Open codes a server holds at most. With 50 open, a guess succeeds with odds of 50 in 2^40; the 200 failures an hour
/// the server allows succeed with odds of about 1 in 650,000 a week, 1 in 12,500 a year.
const MAX_OPEN: i64 = 50;
/// Open pairing codes an account holds at most.
const MAX_OPEN_PAIRINGS: i64 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CodeKind {
  /// The first admin account; printed to the console while the server has no admin.
  Setup,
  /// A server account or a project membership, from an invite with `delivery: code`.
  Invite,
  /// A new device for an account that lost every device.
  Account,
  /// A new device for the account of a device already joined.
  Pairing,
}

impl CodeKind {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Setup => "setup",
      Self::Invite => "invite",
      Self::Account => "account",
      Self::Pairing => "pairing",
    }
  }

  fn parse(value: &str) -> Result<Self, ServerError> {
    [Self::Setup, Self::Invite, Self::Account, Self::Pairing]
      .into_iter()
      .find(|kind| kind.as_str() == value)
      .ok_or(ServerError::StoredData)
  }

  pub const fn lifetime_ms(self) -> i64 {
    match self {
      Self::Setup | Self::Account => 24 * HOUR_MS,
      Self::Invite => 72 * HOUR_MS,
      Self::Pairing => 10 * MINUTE_MS,
    }
  }
}

/// What a new code serves.
pub(crate) struct NewCode {
  pub kind: CodeKind,
  pub invite: Option<InviteId>,
  pub user: Option<UserId>,
  pub made_by: Option<UserId>,
}

/// An open code that matched a redemption.
pub(crate) struct OpenCode {
  row: i64,
  pub kind: CodeKind,
  pub invite: Option<InviteId>,
  pub user: Option<UserId>,
}

/// Records a new code and returns it with its expiry. `409 too_many_codes` at the server's maximum of open codes,
/// `429 rate_limited` at an account's maximum of open pairing codes, which frees up within 10 minutes.
pub(crate) fn create(
  db: &Connection,
  keys: &ServerKeys,
  code: &NewCode,
  now: i64,
) -> Result<(Credential, i64), ServerError> {
  prune(db, now)?;
  let open: i64 = db.query_row("SELECT COUNT(*) FROM codes", [], |row| row.get(0))?;
  if open >= MAX_OPEN {
    return Err(ErrorCode::TooManyCodes.into());
  }
  if code.kind == CodeKind::Pairing {
    let pairings: i64 = db.query_row(
      "SELECT COUNT(*) FROM codes WHERE kind='pairing' AND user_id=?1",
      [code.user.map(|user| user.to_string())],
      |row| row.get(0),
    )?;
    if pairings >= MAX_OPEN_PAIRINGS {
      return Err(ServerError::RateLimited(60));
    }
  }
  let text = crockford(&random::<5>()?);
  let expires_at = now + code.kind.lifetime_ms();
  db.execute(
    "INSERT INTO codes(hmac,kind,invite,user_id,made_by,created_at,expires_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
    params![
      keys.code_tag(&text),
      code.kind.as_str(),
      code.invite.map(|id| id.to_string()),
      code.user.map(|id| id.to_string()),
      code.made_by.map(|id| id.to_string()),
      now,
      expires_at
    ],
  )?;
  Ok((Credential::new(format!("{}-{}", &text[..4], &text[4..])), expires_at))
}

/// Ends the open codes of `kind` for `user` (or with no user), e.g. an earlier setup code when a new one is printed.
pub(crate) fn end(db: &Connection, kind: CodeKind, user: Option<UserId>) -> Result<(), ServerError> {
  db.execute(
    "DELETE FROM codes WHERE kind=?1 AND user_id IS ?2",
    params![kind.as_str(), user.map(|id| id.to_string())],
  )?;
  Ok(())
}

/// The open code `text` names, if any. Every open code is compared, each in constant time.
pub(crate) fn find(db: &Connection, keys: &ServerKeys, text: &str, now: i64) -> Result<Option<OpenCode>, ServerError> {
  let Some(code) = normalize(text) else {
    return Ok(None);
  };
  let mut query =
    db.prepare("SELECT rowid,hmac,kind,invite,user_id FROM codes WHERE used_at IS NULL AND expires_at>?1")?;
  let rows = query
    .query_map([now], |row| {
      Ok((
        row.get::<_, i64>(0)?,
        row.get::<_, Vec<u8>>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, Option<String>>(3)?,
        row.get::<_, Option<String>>(4)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  let mut found = None;
  for (row, tag, kind, invite, user) in rows {
    if keys.code_matches(&code, &tag) && found.is_none() {
      found = Some(OpenCode {
        row,
        kind: CodeKind::parse(&kind)?,
        invite: invite
          .map(|id| id.parse())
          .transpose()
          .map_err(|_| ServerError::StoredData)?,
        user: user
          .map(|id| id.parse())
          .transpose()
          .map_err(|_| ServerError::StoredData)?,
      });
    }
  }
  Ok(found)
}

/// Ends a redeemed code; a code works once.
pub(crate) fn redeem(db: &Connection, code: &OpenCode, now: i64) -> Result<(), ServerError> {
  db.execute("UPDATE codes SET used_at=?1 WHERE rowid=?2", params![now, code.row])?;
  Ok(())
}

/// Ends the open code of an invite, when the invite is revoked.
pub(crate) fn end_invite(db: &Connection, invite: InviteId) -> Result<(), ServerError> {
  db.execute("DELETE FROM codes WHERE invite=?1", [invite.to_string()])?;
  Ok(())
}

/// Removes used and expired codes; they are kept no longer than they can work.
pub(crate) fn prune(db: &Connection, now: i64) -> Result<(), ServerError> {
  db.execute("DELETE FROM codes WHERE used_at IS NOT NULL OR expires_at<=?1", [now])?;
  Ok(())
}

/// A code as typed, in upper case without the hyphen, with Crockford's look-alikes read as the digits they stand for;
/// `None` for anything that cannot be a code.
fn normalize(text: &str) -> Option<String> {
  let code: String = text
    .chars()
    .filter(|c| *c != '-')
    .map(|c| match c.to_ascii_uppercase() {
      'O' => '0',
      'I' | 'L' => '1',
      other => other,
    })
    .collect();
  (code.len() == 8
    && code
      .bytes()
      .all(|byte| orchester_api::certificate::CROCKFORD.contains(&byte)))
  .then_some(code)
}
