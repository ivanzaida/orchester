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

/// The stored root, opened with the key-encryption key; `None` before the first start made it. A key that does not
/// open it is `WrongKeyEncryptionKey`; a row that is not a sealed root key is `StoredData`.
pub(super) async fn load(client: &impl GenericClient, sealing: &KeySealing) -> Result<Option<RootKey>, CloudError> {
  let Some(row) = client
    .query_opt("SELECT public_key, sealed_private_key FROM root_key", &[])
    .await?
  else {
    return Ok(None);
  };
  let public = PublicKey::from_bytes(row.try_get(0)?).map_err(|_| CloudError::StoredData)?;
  let private = sealing.open(&label(&public), row.try_get(1)?).map_err(wrong_key)?;
  let root = RootKey::new(&private).map_err(|_| CloudError::StoredData)?;
  if root.public_key() != public {
    return Err(CloudError::StoredData);
  }
  Ok(Some(root))
}

/// Makes the root of a database that holds no keys at all. Signing keys or key sets without a root mean the root row
/// was lost or deleted alone: the cloud refuses rather than make a root beside keys another root vouched for
/// (docs/cloud-hosting.md, "Losing the key-encryption key").
async fn create(client: &impl GenericClient, sealing: &KeySealing, now: i64) -> Result<RootKey, CloudError> {
  let keys: bool = client
    .query_one(
      "SELECT EXISTS (SELECT 1 FROM signing_keys) OR EXISTS (SELECT 1 FROM key_sets)",
      &[],
    )
    .await?
    .try_get(0)?;
  if keys {
    return Err(CloudError::KeysWithoutRoot);
  }
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
  audit(client, None, "root_key_created", &public.fingerprint(), None, now).await?;
  Ok(root)
}

/// A sealed key the key-encryption key does not open means the key is not the one it was sealed with.
fn wrong_key(error: CloudError) -> CloudError {
  match error {
    CloudError::KeyEncryption => CloudError::WrongKeyEncryptionKey,
    other => other,
  }
}

/// What the root's sealed private half is bound to, apart from every signing key's kid.
fn label(public: &PublicKey) -> String {
  format!("root {}", public.kid())
}
