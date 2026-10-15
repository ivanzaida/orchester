//! The host commands of the key ceremony: `keys rotate`, `keys prepare` and `keys publish` run where the cloud's
//! database is; `keys sign` runs on the offline machine with the root key and needs no cloud (`main.rs`).

use orchester_api::identity::{KeySetStatement, ListedKey, PrivateKey, PublicKey, SigningKey, VerifiedKeySet};

use super::{LISTED_AFTER_MS, SIGNS_MS};
use crate::{
  Cloud,
  clock::seconds,
  db::{audit, millis, time},
  error::CloudError,
};

impl Cloud {
  /// Makes a new signing key, usable from now for 30 days once a published key set lists it. The previous key keeps
  /// signing until then, so tokens never stop between `rotate` and `publish`.
  pub async fn host_rotate_keys(&self) -> Result<String, CloudError> {
    let private = PrivateKey::generate()?;
    let key = SigningKey::new(&private)?;
    let sealed = self.inner.sealing.seal(key.kid(), &private)?;
    let now = self.now();
    let client = self.inner.db.client().await?;
    client
      .execute(
        "INSERT INTO signing_keys (kid, public_key, sealed_private_key, not_before, signs_until, listed_until, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $4)",
        &[
          &key.kid(),
          &key.public_key().as_bytes().as_slice(),
          &sealed,
          &time(now),
          &time(now + SIGNS_MS),
          &time(now + SIGNS_MS + LISTED_AFTER_MS),
        ],
      )
      .await?;
    audit(&client, None, "signing_key_created", key.kid(), None, now).await?;
    Ok(key.kid().to_owned())
  }

  /// The statement for the root key to sign: every key still listed, valid for 30 days from now.
  pub async fn host_prepare_key_set(&self) -> Result<KeySetStatement, CloudError> {
    let now = self.now();
    let rows = self
      .inner
      .db
      .client()
      .await?
      .query(
        "SELECT kid, public_key, not_before, signs_until, listed_until FROM signing_keys
         WHERE listed_until > $1 ORDER BY not_before, kid",
        &[&time(now)],
      )
      .await?;
    let keys = rows
      .iter()
      .map(|row| {
        Ok(ListedKey {
          kid: row.try_get(0)?,
          public_key: PublicKey::from_bytes(row.try_get(1)?)?.encoded(),
          not_before: seconds(millis(row.try_get(2)?)?),
          signs_until: seconds(millis(row.try_get(3)?)?),
          listed_until: seconds(millis(row.try_get(4)?)?),
        })
      })
      .collect::<Result<Vec<_>, CloudError>>()?;
    if keys.is_empty() {
      return Err(CloudError::Configuration(
        "no signing key is listed; run `orchester-cloud keys rotate` first",
      ));
    }
    Ok(KeySetStatement::new(
      self.inner.config.public_url.clone(),
      keys,
      seconds(now),
    ))
  }

  /// Stores a key set the root key signed, after checking it verifies with the configured root, is this cloud's,
  /// and lists only keys this cloud holds, each with its own public key.
  pub async fn host_publish_key_set(&self, compact: &str) -> Result<(), CloudError> {
    let now = self.now();
    let verified = VerifiedKeySet::verify(compact.trim(), &self.inner.config.root_public_key, seconds(now))?;
    let statement = verified.statement();
    if statement.iss != self.inner.config.public_url {
      return Err(CloudError::Configuration(
        "the key set was prepared for another PUBLIC_URL",
      ));
    }
    let client = self.inner.db.client().await?;
    for listed in &statement.keys {
      let stored = client
        .query_opt("SELECT public_key FROM signing_keys WHERE kid = $1", &[&listed.kid])
        .await?
        .map(|row| row.try_get::<_, Vec<u8>>(0))
        .transpose()?;
      let matches =
        stored.is_some_and(|bytes| PublicKey::from_bytes(&bytes).is_ok_and(|key| key.encoded() == listed.public_key));
      if !matches {
        return Err(CloudError::Configuration(
          "the key set lists a key this cloud does not hold",
        ));
      }
    }
    client
      .execute(
        "INSERT INTO key_sets (issued_at, expires_at, compact, published_at) VALUES ($1, $2, $3, $4)
         ON CONFLICT (issued_at) DO NOTHING",
        &[
          &time(statement.issued_at * 1000),
          &time(statement.expires_at * 1000),
          &compact.trim(),
          &time(now),
        ],
      )
      .await?;
    audit(
      &client,
      None,
      "key_set_published",
      &statement.issued_at.to_string(),
      None,
      now,
    )
    .await
  }
}
