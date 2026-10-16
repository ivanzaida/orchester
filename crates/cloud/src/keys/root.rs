//! The cloud's root key: made at its first start, kept in the database sealed with the key-encryption key, and used
//! only to sign key sets. Team servers pin its public half when they link.

use deadpool_postgres::GenericClient;
use orchester_api::identity::{PrivateKey, PublicKey, RootKey};

use super::{KeySealing, LOCK};
use crate::{
  db::{Database, audit, time},
  error::CloudError,
};

/// Loads the root key, or makes and stores one when the database has none. Replicas that start together take turns on
/// the lock, so the second finds the first one's root; the table holds one row at most, so a second root could not
/// be stored anyway. A root the key-encryption key does not open stops the start: the cloud never replaces it.
pub(crate) async fn open(db: &Database, sealing: &KeySealing, now: i64) -> Result<PublicKey, CloudError> {
  let mut client = db.client().await?;
  let transaction = client.transaction().await?;
  transaction
    .execute("SELECT pg_advisory_xact_lock($1)", &[&LOCK])
    .await?;
  let root = match load(&transaction, sealing).await? {
    Some(root) => root,
    None => create(&transaction, sealing, now).await?,
  };
  transaction.commit().await?;
  Ok(root.public_key())
}

/// The stored root, opened with the key-encryption key; `None` before the first start made it.
pub(super) async fn load(client: &impl GenericClient, sealing: &KeySealing) -> Result<Option<RootKey>, CloudError> {
  let Some(row) = client
    .query_opt("SELECT public_key, sealed_private_key FROM root_key", &[])
    .await?
  else {
    return Ok(None);
  };
  let public = PublicKey::from_bytes(row.try_get(0)?)?;
  let private = sealing
    .open(&label(&public), row.try_get(1)?)
    .map_err(|_| CloudError::WrongKeyEncryptionKey)?;
  let root = RootKey::new(&private)?;
  if root.public_key() != public {
    return Err(CloudError::StoredData);
  }
  Ok(Some(root))
}

async fn create(client: &impl GenericClient, sealing: &KeySealing, now: i64) -> Result<RootKey, CloudError> {
  let private = PrivateKey::generate()?;
  let root = RootKey::new(&private)?;
  let public = root.public_key();
  let sealed = sealing.seal(&label(&public), &private)?;
  client
    .execute(
      "INSERT INTO root_key (public_key, sealed_private_key, created_at) VALUES ($1, $2, $3)",
      &[&public.as_bytes().as_slice(), &sealed, &time(now)],
    )
    .await?;
  // Key sets an earlier root signed (a database from before the cloud kept its own root) never verify with this
  // one; the servers that pinned that root pin this one again.
  let earlier = client.execute("DELETE FROM key_sets", &[]).await?;
  if earlier > 0 {
    tracing::warn!(
      root_public_key = %public.encoded(),
      root_fingerprint = %public.fingerprint(),
      "key sets of an earlier root removed; team servers linked before must pin the new root"
    );
  }
  audit(client, None, "root_key_created", &public.fingerprint(), None, now).await?;
  Ok(root)
}

/// What the root's sealed private half is bound to, apart from every signing key's kid.
fn label(public: &PublicKey) -> String {
  format!("root {}", public.kid())
}
