//! One-time confirmation and reset links (`email_links`): only a token's SHA-256 is kept, and using a link is one
//! statement, so two finishes of one link cannot both pass.

use deadpool_postgres::GenericClient;
use orchester_api::Credential;
use orchester_core::id::CloudAccountId;

use crate::{
  clock::{DAY_MS, HOUR_MS},
  db::time,
  error::CloudError,
  tokens::{random_token, token_hash, well_formed},
};

const CONFIRM_MS: i64 = DAY_MS;
const RESET_MS: i64 = HOUR_MS;

#[derive(Clone, Copy)]
pub(super) enum LinkPurpose {
  Confirm,
  Reset,
}

impl LinkPurpose {
  const fn as_str(self) -> &'static str {
    match self {
      Self::Confirm => "confirm",
      Self::Reset => "reset",
    }
  }
}

/// Records a one-time link for the account and returns its token, which only the email carries.
pub(super) async fn new_link(
  client: &impl GenericClient,
  account: CloudAccountId,
  purpose: LinkPurpose,
  now: i64,
) -> Result<Credential, CloudError> {
  let token = random_token("")?;
  let lifetime = match purpose {
    LinkPurpose::Confirm => CONFIRM_MS,
    LinkPurpose::Reset => RESET_MS,
  };
  client
    .execute(
      "INSERT INTO email_links (token_hash, account, purpose, created_at, expires_at) VALUES ($1, $2, $3, $4, $5)",
      &[
        &token_hash(&token),
        &account.as_uuid(),
        &purpose.as_str(),
        &time(now),
        &time(now + lifetime),
      ],
    )
    .await?;
  Ok(token)
}

/// Marks an unused, unexpired link used in one statement, so two finishes of one link cannot both pass.
pub(super) async fn use_link(
  client: &impl GenericClient,
  token: &Credential,
  purpose: LinkPurpose,
  now: i64,
) -> Result<Option<CloudAccountId>, CloudError> {
  if !well_formed(token.expose(), "") {
    return Ok(None);
  }
  let row = client
    .query_opt(
      "UPDATE email_links SET used_at = $1
       WHERE token_hash = $2 AND purpose = $3 AND used_at IS NULL AND expires_at > $1 RETURNING account",
      &[&time(now), &token_hash(token), &purpose.as_str()],
    )
    .await?;
  Ok(match row {
    Some(row) => Some(CloudAccountId::from_uuid(row.try_get(0)?)),
    None => None,
  })
}

/// Whether a link could be used now, without using it: lets a reset refuse a bad link before hashing a password.
pub(super) async fn link_usable(
  client: &impl GenericClient,
  token: &Credential,
  purpose: LinkPurpose,
  now: i64,
) -> Result<bool, CloudError> {
  if !well_formed(token.expose(), "") {
    return Ok(false);
  }
  Ok(
    client
      .query_opt(
        "SELECT 1 FROM email_links WHERE token_hash = $1 AND purpose = $2 AND used_at IS NULL AND expires_at > $3",
        &[&token_hash(token), &purpose.as_str(), &time(now)],
      )
      .await?
      .is_some(),
  )
}
