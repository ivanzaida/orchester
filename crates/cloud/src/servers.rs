//! The team servers the cloud knows (`docs/architecture/server.md`, "Orchester Cloud", "Linking a team server at
//! setup"). A server signs a link statement with its own key; its owner, signed in, carries it here, and the cloud
//! registers the server for that account. The statement names the key that verifies it, and the cloud keeps that key
//! to verify the server's invites. A server id stays with its first key and its owner: another key, or another account
//! while the owner has one, is a conflict.

use deadpool_postgres::GenericClient;
use orchester_api::{
  cloud::{AccountServer, CloudErrorCode, CloudServer, LINK_STATEMENT_TYPE, LinkStatement, ServerLink},
  identity::{CLOCK_SKEW_SECS, PublicKey, STATEMENT_FRESH_SECS, unverified_statement, verify_statement},
};
use orchester_core::id::TeamServerId;
use tokio_postgres::Row;

use crate::{
  Cloud,
  accounts::{Authenticated, input},
  clock::seconds,
  db::{audit, millis, time},
  error::CloudError,
};

const MAX_NAME_CHARS: usize = 100;
const MAX_ADDRESS_CHARS: usize = 2048;

impl Cloud {
  /// Registers the server a fresh link statement names, owned by the signed-in account; linking again updates its
  /// name, address and certificate.
  pub(crate) async fn link_server(&self, auth: Authenticated, input: ServerLink) -> Result<AccountServer, CloudError> {
    let invalid = |_| CloudError::from(CloudErrorCode::Invalid);
    let named: LinkStatement = unverified_statement(&input.link, LINK_STATEMENT_TYPE).map_err(invalid)?;
    let key = PublicKey::parse(&named.public_key).map_err(invalid)?;
    let link: LinkStatement = verify_statement(&input.link, LINK_STATEMENT_TYPE, &key).map_err(invalid)?;
    let now = self.now();
    let at = seconds(now);
    if link.issued_at < at - STATEMENT_FRESH_SECS || link.issued_at > at + CLOCK_SKEW_SECS {
      return Err(CloudErrorCode::Invalid.into());
    }
    let server = CloudServer {
      id: link.server,
      name: input::text(&link.name, MAX_NAME_CHARS)?,
      address: address(&link.address)?,
      fingerprint: link.fingerprint.as_deref().map(fingerprint).transpose()?,
    };
    let client = self.inner.db.client().await?;
    let linked = client
      .query_opt(
        "INSERT INTO servers (id, owner, name, address, fingerprint, public_key, linked_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (id) DO UPDATE SET owner = EXCLUDED.owner, name = EXCLUDED.name, address = EXCLUDED.address,
           fingerprint = EXCLUDED.fingerprint, linked_at = EXCLUDED.linked_at
         WHERE servers.public_key = EXCLUDED.public_key AND (servers.owner IS NULL OR servers.owner = EXCLUDED.owner)
         RETURNING linked_at",
        &[
          &server.id.as_uuid(),
          &auth.account.as_uuid(),
          &server.name,
          &server.address,
          &server.fingerprint,
          &key.as_bytes().as_slice(),
          &time(now),
        ],
      )
      .await?
      .ok_or(CloudErrorCode::Conflict)?;
    audit(
      &client,
      Some(auth.account),
      "server_linked",
      &server.id.to_string(),
      None,
      now,
    )
    .await?;
    Ok(AccountServer {
      server,
      owner: true,
      joined_at: millis(linked.try_get(0)?)?,
    })
  }

  /// The servers the account owns or joined, in the order it came to them.
  pub(crate) async fn servers(&self, auth: Authenticated) -> Result<Vec<AccountServer>, CloudError> {
    let client = self.inner.db.client().await?;
    let rows = client
      .query(
        "SELECT s.id, s.name, s.address, s.fingerprint, coalesce(s.owner = $1, false) AS owned,
           CASE WHEN s.owner = $1 THEN s.linked_at ELSE j.joined_at END AS since
         FROM servers s LEFT JOIN server_accounts j ON j.server = s.id AND j.account = $1
         WHERE s.owner = $1 OR j.account IS NOT NULL
         ORDER BY since, s.id",
        &[&auth.account.as_uuid()],
      )
      .await?;
    rows
      .iter()
      .map(|row| {
        Ok(AccountServer {
          server: server_row(row, 0)?,
          owner: row.try_get(4)?,
          joined_at: millis(row.try_get(5)?)?,
        })
      })
      .collect()
  }
}

/// A server as an app needs it, from the columns `id, name, address, fingerprint` starting at `first`.
pub(crate) fn server_row(row: &Row, first: usize) -> Result<CloudServer, CloudError> {
  Ok(CloudServer {
    id: TeamServerId::from_uuid(row.try_get(first)?),
    name: row.try_get(first + 1)?,
    address: row.try_get(first + 2)?,
    fingerprint: row.try_get(first + 3)?,
  })
}

/// Records that the account joined the server, once.
pub(crate) async fn joined(
  client: &impl GenericClient,
  server: TeamServerId,
  auth: Authenticated,
  now: i64,
) -> Result<(), CloudError> {
  client
    .execute(
      "INSERT INTO server_accounts (server, account, joined_at) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
      &[&server.as_uuid(), &auth.account.as_uuid(), &time(now)],
    )
    .await?;
  Ok(())
}

/// An `http(s)` base address without spaces, a query or a trailing slash, as an app parses it.
fn address(value: &str) -> Result<String, CloudError> {
  let value = value.trim();
  let scheme = value.starts_with("https://") || value.starts_with("http://");
  let plain = !value.chars().any(|c| c.is_whitespace() || c.is_control()) && !value.contains(['?', '#']);
  if !scheme || !plain || value.ends_with('/') || value.chars().count() > MAX_ADDRESS_CHARS {
    return Err(CloudErrorCode::Invalid.into());
  }
  Ok(value.to_owned())
}

/// A certificate's SHA-256 as 64 lowercase hex characters.
fn fingerprint(value: &str) -> Result<String, CloudError> {
  if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
    return Err(CloudErrorCode::Invalid.into());
  }
  Ok(value.to_owned())
}
