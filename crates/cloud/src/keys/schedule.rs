//! The key schedule, kept by the cloud itself at its start and every minute while it serves: a new signing key once
//! none would sign a week from now, and a new key set signed by the root after each rotation, at least weekly, and
//! whenever the newest one no longer verifies or names another public URL.

use std::time::SystemTime;

use deadpool_postgres::GenericClient;
use orchester_api::identity::{KeySetStatement, ListedKey, PrivateKey, PublicKey, RootKey, SigningKey, VerifiedKeySet};

use super::{KEY_SET_EVERY_MS, KeySealing, LISTED_AFTER_MS, LOCK, OVERLAP_MS, ROTATE_AHEAD_MS, SIGNS_MS, root};
use crate::{
  Cloud,
  clock::seconds,
  db::{audit, millis, time},
  error::CloudError,
};

impl Cloud {
  /// Rotates the signing key and signs a key set when the schedule asks for it now, and does nothing otherwise.
  /// Replicas take turns on the lock, so each step happens once.
  pub async fn keep_keys(&self) -> Result<(), CloudError> {
    let now = self.now();
    let mut client = self.client().await?;
    let transaction = client.transaction().await?;
    transaction
      .execute("SELECT pg_advisory_xact_lock($1)", &[&LOCK])
      .await?;
    let rotated = rotate_if_due(&transaction, &self.inner.sealing, now).await?;
    if rotated || self.key_set_due(&transaction, now).await? {
      let root = root::load(&transaction, &self.inner.sealing)
        .await?
        .ok_or(CloudError::StoredData)?;
      self.publish(&transaction, &root, now).await?;
    }
    transaction.commit().await?;
    Ok(())
  }

  /// True when there is no key set, the newest is a week old, or it does not verify with this cloud's root and name
  /// its public URL.
  async fn key_set_due(&self, client: &impl GenericClient, now: i64) -> Result<bool, CloudError> {
    let Some(row) = client
      .query_opt(
        "SELECT issued_at, compact FROM key_sets ORDER BY issued_at DESC LIMIT 1",
        &[],
      )
      .await?
    else {
      return Ok(true);
    };
    if millis(row.try_get(0)?)? <= now - KEY_SET_EVERY_MS {
      return Ok(true);
    }
    let current = VerifiedKeySet::verify(row.try_get(1)?, &self.inner.root, seconds(now))
      .is_ok_and(|set| set.statement().iss == self.inner.config.public_url);
    Ok(!current)
  }

  /// Signs a key set of every key still listed, valid for 30 days from now, and publishes it.
  async fn publish(&self, client: &impl GenericClient, root: &RootKey, now: i64) -> Result<(), CloudError> {
    let statement = KeySetStatement::new(
      self.inner.config.public_url.clone(),
      listed_keys(client, now).await?,
      seconds(now),
    );
    let compact = root.sign_key_set(&statement, seconds(now))?;
    // A second key set in the same second (a clock set back) replaces the first; both are this root's.
    client
      .execute(
        "INSERT INTO key_sets (issued_at, expires_at, compact, published_at) VALUES ($1, $2, $3, $4)
         ON CONFLICT (issued_at) DO UPDATE
         SET expires_at = excluded.expires_at, compact = excluded.compact, published_at = excluded.published_at",
        &[
          &time(statement.issued_at * 1000),
          &time(statement.expires_at * 1000),
          &compact,
          &time(now),
        ],
      )
      .await?;
    audit(
      client,
      None,
      "key_set_published",
      &statement.issued_at.to_string(),
      None,
      now,
    )
    .await
  }
}

/// Makes a new signing key once no key would sign a week from now. While another key signs, the new one starts only
/// two days later, so team servers have it before tokens signed with it arrive (review 1, L4); the previous key signs
/// until then. True when it made one.
async fn rotate_if_due(client: &impl GenericClient, sealing: &KeySealing, now: i64) -> Result<bool, CloudError> {
  if signing_at(client, now + ROTATE_AHEAD_MS).await? {
    return Ok(false);
  }
  let not_before = if signing_at(client, now).await? {
    now + OVERLAP_MS
  } else {
    now
  };
  let private = PrivateKey::generate()?;
  let key = SigningKey::new(&private)?;
  let sealed = sealing.seal(key.kid(), &private)?;
  client
    .execute(
      "INSERT INTO signing_keys (kid, public_key, sealed_private_key, not_before, signs_until, listed_until, created_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7)",
      &[
        &key.kid(),
        &key.public_key().as_bytes().as_slice(),
        &sealed,
        &time(not_before),
        &time(not_before + SIGNS_MS),
        &time(not_before + SIGNS_MS + LISTED_AFTER_MS),
        &time(now),
      ],
    )
    .await?;
  audit(client, None, "signing_key_created", key.kid(), None, now).await?;
  Ok(true)
}

async fn signing_at(client: &impl GenericClient, at: i64) -> Result<bool, CloudError> {
  let row = client
    .query_opt(
      "SELECT 1 FROM signing_keys WHERE not_before <= $1 AND signs_until > $1",
      &[&time(at)],
    )
    .await?;
  Ok(row.is_some())
}

/// Every key still listed at `now`, oldest first.
async fn listed_keys(client: &impl GenericClient, now: i64) -> Result<Vec<ListedKey>, CloudError> {
  let rows = client
    .query(
      "SELECT kid, public_key, not_before, signs_until, listed_until FROM signing_keys
       WHERE listed_until > $1 ORDER BY not_before, kid",
      &[&time(now)],
    )
    .await?;
  rows
    .iter()
    .map(|row| {
      let at = |index: usize| -> Result<i64, CloudError> { Ok(seconds(millis(row.try_get::<_, SystemTime>(index)?)?)) };
      Ok(ListedKey {
        kid: row.try_get(0)?,
        public_key: PublicKey::from_bytes(row.try_get(1)?)?.encoded(),
        not_before: at(2)?,
        signs_until: at(3)?,
        listed_until: at(4)?,
      })
    })
    .collect()
}
