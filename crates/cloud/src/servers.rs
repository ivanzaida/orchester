//! The team server registry (`docs/architecture/server.md`, "Linking a team server at setup"): a server is linked to
//! the account of the admin who set it up, from the link statement the server signed with its own key and the admin's
//! browser carried here. The cloud keeps its id, name, address, certificate fingerprint and public key, so it can name
//! the server to people, check its invites, and issue identity tokens for it.

use deadpool_postgres::GenericClient;
use orchester_api::{
  certificate::CertificatePin,
  cloud::{AccountServer, CloudErrorCode, CloudServer, RegisteredServer, ServerLink, ServerLinked, TokenPurpose},
  identity::{CLOCK_SKEW_SECS, PublicKey, read_statement, verify_statement},
  statements::{LINK_STATEMENT_TYPE, LinkStatement},
};
use orchester_core::id::TeamServerId;
use tokio_postgres::Row;
use uuid::Uuid;

use crate::{
  Cloud,
  accounts::Authenticated,
  clock::{MINUTE_MS, seconds},
  db::{audit, millis, time},
  error::CloudError,
};

/// A link statement older than this is refused: the admin's browser carries it here within a minute or two.
const STATEMENT_FRESH_MS: i64 = 10 * MINUTE_MS;
const MAX_NAME_CHARS: usize = 128;
const COLUMNS: &str = "id, name, address, fingerprint";
/// Linked servers one account may own.
const MAX_OWNED: i64 = 5;
/// A statement's nonce: 16 to 64 base64url or hex characters.
const NONCE: std::ops::RangeInclusive<usize> = 16..=64;

impl Cloud {
  /// `POST /servers`: links the server that signed `link` to the signed-in account, and answers with what the server
  /// pins (the root key and the newest key set) and a token that links the admin's account there. The server's id is
  /// its key's, so only that key's holder links it; each statement works once, and a newer one takes the server over
  /// from whoever linked it before (its admins moved it to another account).
  pub(crate) async fn link_server(&self, auth: Authenticated, input: ServerLink) -> Result<ServerLinked, CloudError> {
    let statement = verified_statement(&input.link, self.now())?;
    let now = self.now();
    let mut client = self.client().await?;
    let transaction = client.transaction().await?;
    let id = statement.server.as_uuid();
    use_statement(&transaction, &statement, now).await?;
    owned_below_cap(&transaction, auth, id).await?;
    if !upsert(&transaction, &statement, auth, now).await? {
      return Err(CloudErrorCode::Conflict.into());
    }
    joined(&transaction, id, auth, now).await?;
    audit(
      &transaction,
      Some(auth.account),
      "server_linked",
      &id.to_string(),
      None,
      now,
    )
    .await?;
    let key_set = self.key_set(&transaction).await?;
    transaction.commit().await?;
    let issued = self
      .issue_for(auth, statement.server.to_string(), Some(TokenPurpose::Link))
      .await?;
    Ok(ServerLinked {
      server: registered(server(&statement)),
      root_key: self.inner.config.root_public_key.encoded(),
      key_set,
      identity_token: issued.identity_token,
      expires_at: issued.expires_at,
    })
  }

  /// `GET /servers/{id}`: a linked server, for a page that names it and asks the person to compare its check code.
  pub(crate) async fn server(&self, id: TeamServerId) -> Result<RegisteredServer, CloudError> {
    let client = self.client().await?;
    let row = linked(&client, id.as_uuid()).await?.ok_or(CloudErrorCode::NotFound)?;
    Ok(registered(server_row(&row)?))
  }

  /// `GET /servers`: the linked servers the account owns or joined, oldest first.
  pub(crate) async fn servers(&self, auth: Authenticated) -> Result<Vec<AccountServer>, CloudError> {
    let rows = self
      .client()
      .await?
      .query(
        "SELECT s.id, s.name, s.address, s.fingerprint, coalesce(s.owner = $1, false),
           least(coalesce(j.first_joined, s.linked_at), s.linked_at)
         FROM servers s LEFT JOIN server_accounts j ON j.server = s.id AND j.account = $1
         WHERE s.unlinked_at IS NULL AND (s.owner = $1 OR j.account IS NOT NULL)
         ORDER BY 6, s.id",
        &[&auth.account.as_uuid()],
      )
      .await?;
    rows
      .iter()
      .map(|row| {
        Ok(AccountServer {
          server: server_row(row)?,
          owner: row.try_get(4)?,
          joined_at: millis(row.try_get(5)?)?,
        })
      })
      .collect()
  }
}

/// Uses the statement up: a second use (a replay, from a history or a log) is a `409 conflict`.
async fn use_statement(client: &impl GenericClient, statement: &LinkStatement, now: i64) -> Result<(), CloudError> {
  let used = client
    .execute(
      "INSERT INTO used_link_statements (nonce, expires_at) VALUES ($1, $2) ON CONFLICT DO NOTHING",
      &[
        &statement.nonce,
        &time(now + STATEMENT_FRESH_MS + CLOCK_SKEW_SECS * 1000),
      ],
    )
    .await?;
  if used == 0 {
    return Err(CloudErrorCode::Conflict.into());
  }
  Ok(())
}

/// One account owns at most [`MAX_OWNED`] linked servers; linking one it owns already does not count.
async fn owned_below_cap(client: &impl GenericClient, auth: Authenticated, id: Uuid) -> Result<(), CloudError> {
  let owned: i64 = client
    .query_one(
      "SELECT count(*) FROM servers WHERE owner = $1 AND unlinked_at IS NULL AND id <> $2",
      &[&auth.account.as_uuid(), &id],
    )
    .await?
    .try_get(0)?;
  if owned >= MAX_OWNED {
    return Err(CloudErrorCode::TooManyServers.into());
  }
  Ok(())
}

/// Links the server to the account: a new server, one unlinked, or one whose stored statement is older than this
/// one. `false` when a statement at least as new linked it already. The key is the stored one by construction: the
/// id is the key's.
async fn upsert(
  client: &impl GenericClient,
  statement: &LinkStatement,
  auth: Authenticated,
  now: i64,
) -> Result<bool, CloudError> {
  let issued = time(statement.issued_at * 1000);
  let changed = client
    .execute(
      "INSERT INTO servers (id, owner, name, address, fingerprint, public_key, linked_at, statement_issued_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
       ON CONFLICT (id) DO UPDATE SET owner = $2, name = $3, address = $4, fingerprint = $5, public_key = $6,
         linked_at = $7, statement_issued_at = $8, unlinked_at = NULL
       WHERE servers.public_key = $6 AND (servers.unlinked_at IS NOT NULL OR servers.statement_issued_at IS NULL
         OR servers.statement_issued_at < $8)",
      &[
        &statement.server.as_uuid(),
        &auth.account.as_uuid(),
        &statement.name.trim(),
        &statement.address,
        &statement.fingerprint,
        &statement.public_key,
        &time(now),
        &issued,
      ],
    )
    .await?;
  Ok(changed == 1)
}

/// The linked server `id`, if any.
pub(crate) async fn linked(client: &impl GenericClient, id: Uuid) -> Result<Option<Row>, CloudError> {
  Ok(
    client
      .query_opt(
        &format!("SELECT {COLUMNS}, public_key FROM servers WHERE id = $1 AND unlinked_at IS NULL"),
        &[&id],
      )
      .await?,
  )
}

/// Records that `auth`'s account uses the server, once.
pub(crate) async fn joined(
  client: &impl GenericClient,
  server: Uuid,
  auth: Authenticated,
  now: i64,
) -> Result<(), CloudError> {
  client
    .execute(
      "INSERT INTO server_accounts (server, account, first_joined) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
      &[&server, &auth.account.as_uuid(), &time(now)],
    )
    .await?;
  Ok(())
}

/// A link statement its own key signed, fresh, naming a server the cloud can show and an app can reach.
fn verified_statement(link: &str, now: i64) -> Result<LinkStatement, CloudError> {
  let invalid = || CloudError::from(CloudErrorCode::Invalid);
  let unverified: LinkStatement = read_statement(link, LINK_STATEMENT_TYPE).map_err(|_| invalid())?;
  let key = PublicKey::parse(&unverified.public_key).map_err(|_| invalid())?;
  let statement: LinkStatement = verify_statement(link, LINK_STATEMENT_TYPE, &key).map_err(|_| invalid())?;
  // The id is the key's: a statement for another server's id with a key of one's own is refused here.
  if statement.server != key.server_id() {
    return Err(invalid());
  }
  let nonce_ok = NONCE.contains(&statement.nonce.len())
    && statement
      .nonce
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
  let issued = statement.issued_at;
  let fresh = seconds(now - STATEMENT_FRESH_MS)..=seconds(now) + CLOCK_SKEW_SECS;
  let name = statement.name.trim();
  let fingerprint_ok = statement.fingerprint.as_deref().is_none_or(|fingerprint| {
    CertificatePin::parse(fingerprint).is_some() && fingerprint == fingerprint.to_lowercase()
  });
  if !fresh.contains(&issued)
    || !nonce_ok
    || name.is_empty()
    || name.chars().count() > MAX_NAME_CHARS
    || !valid_address(&statement.address)
    || !fingerprint_ok
  {
    return Err(invalid());
  }
  Ok(statement)
}

/// An `https://` or `http://` base address without a path, as the team server's `address` is.
fn valid_address(address: &str) -> bool {
  let rest = address
    .strip_prefix("https://")
    .or_else(|| address.strip_prefix("http://"));
  rest.is_some_and(|host| {
    !host.is_empty() && host.len() <= 255 && !host.contains(['/', '?', '#', '@']) && !host.contains(char::is_whitespace)
  })
}

fn server(statement: &LinkStatement) -> CloudServer {
  CloudServer {
    id: statement.server,
    name: statement.name.trim().to_owned(),
    address: statement.address.clone(),
    fingerprint: statement.fingerprint.clone(),
  }
}

fn server_row(row: &Row) -> Result<CloudServer, CloudError> {
  Ok(CloudServer {
    id: TeamServerId::from_uuid(row.try_get(0)?),
    name: row.try_get(1)?,
    address: row.try_get(2)?,
    fingerprint: row.try_get(3)?,
  })
}

fn registered(server: CloudServer) -> RegisteredServer {
  let check_code = server
    .fingerprint
    .as_deref()
    .and_then(CertificatePin::parse)
    .map(|pin| pin.check_code());
  RegisteredServer { server, check_code }
}
