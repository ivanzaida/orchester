//! One-time confirmation and reset links (`email_links`): only a token's SHA-256 is kept, and using a link is one
//! statement, so two finishes of one link cannot both pass.

use deadpool_postgres::GenericClient;
use orchester_api::Credential;
use orchester_core::id::CloudAccountId;

use crate::{
  clock::{DAY_MS, HOUR_MS},
  db::time,
  error::CloudError,
  passwords::PasswordHash,
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

/// What a sign-up asked for: the account takes it only when the link sent for that sign-up is confirmed (review 1,
/// H1), so whoever signed up first with someone else's email never sets that account's password.
pub(super) struct Pending<'a> {
  pub hash: &'a PasswordHash,
  pub display_name: &'a str,
}

pub(super) enum NewLink<'a> {
  Confirm(Pending<'a>),
  Reset,
}

/// A link just used: for a confirmation link, the sign-up it was sent for.
pub(super) struct UsedLink {
  pub hash: Option<PasswordHash>,
  pub display_name: Option<String>,
}

/// Records a one-time link for the account and returns its token, which only the email carries.
pub(super) async fn new_link(
  client: &impl GenericClient,
  account: CloudAccountId,
  link: NewLink<'_>,
  now: i64,
) -> Result<Credential, CloudError> {
  let token = random_token("")?;
  let (purpose, lifetime, hash, display_name) = match &link {
    NewLink::Confirm(pending) => (
      LinkPurpose::Confirm,
      CONFIRM_MS,
      Some(pending.hash.expose()),
      Some(pending.display_name),
    ),
    NewLink::Reset => (LinkPurpose::Reset, RESET_MS, None, None),
  };
  client
    .execute(
      "INSERT INTO email_links (token_hash, account, purpose, created_at, expires_at, password_hash, display_name)
       VALUES ($1, $2, $3, $4, $5, $6, $7)",
      &[
        &token_hash(&token),
        &account.as_uuid(),
        &purpose.as_str(),
        &time(now),
        &time(now + lifetime),
        &hash,
        &display_name,
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
) -> Result<Option<UsedLink>, CloudError> {
  if !well_formed(token.expose(), "") {
    return Ok(None);
  }
  let row = client
    .query_opt(
      "UPDATE email_links SET used_at = $1
       WHERE token_hash = $2 AND purpose = $3 AND used_at IS NULL AND expires_at > $1
       RETURNING password_hash, display_name",
      &[&time(now), &token_hash(token), &purpose.as_str()],
    )
    .await?;
  let Some(row) = row else {
    return Ok(None);
  };
  Ok(Some(UsedLink {
    hash: row.try_get::<_, Option<String>>(0)?.map(PasswordHash::stored),
    display_name: row.try_get(1)?,
  }))
}

/// Ends the account's open links of `purpose`, or of every purpose: a newer sign-up ends older confirmation links,
/// and a password set by a reset or a change ends them all.
pub(super) async fn end_links(
  client: &impl GenericClient,
  account: CloudAccountId,
  purpose: Option<LinkPurpose>,
  now: i64,
) -> Result<(), CloudError> {
  client
    .execute(
      "UPDATE email_links SET used_at = $1
       WHERE account = $2 AND used_at IS NULL AND ($3::text IS NULL OR purpose = $3)",
      &[&time(now), &account.as_uuid(), &purpose.map(LinkPurpose::as_str)],
    )
    .await?;
  Ok(())
}

/// The account of an unused, unexpired link, and for a confirmation link its sign-up's password hash, without using
/// the link.
pub(super) async fn pending_link(
  client: &impl GenericClient,
  token: &Credential,
  purpose: LinkPurpose,
  now: i64,
) -> Result<Option<(CloudAccountId, Option<PasswordHash>)>, CloudError> {
  if !well_formed(token.expose(), "") {
    return Ok(None);
  }
  let row = client
    .query_opt(
      "SELECT account, password_hash FROM email_links
       WHERE token_hash = $1 AND purpose = $2 AND used_at IS NULL AND expires_at > $3",
      &[&token_hash(token), &purpose.as_str(), &time(now)],
    )
    .await?;
  let Some(row) = row else {
    return Ok(None);
  };
  Ok(Some((
    CloudAccountId::from_uuid(row.try_get(0)?),
    row.try_get::<_, Option<String>>(1)?.map(PasswordHash::stored),
  )))
}

/// Locks the account row for the rest of the transaction; every change of an account and its links takes the
/// account row first, so two of them never wait on each other in opposite orders (review 2, N3). False when the
/// account is gone.
pub(super) async fn lock_account(client: &impl GenericClient, account: CloudAccountId) -> Result<bool, CloudError> {
  Ok(
    client
      .query_opt("SELECT 1 FROM accounts WHERE id = $1 FOR UPDATE", &[&account.as_uuid()])
      .await?
      .is_some(),
  )
}
