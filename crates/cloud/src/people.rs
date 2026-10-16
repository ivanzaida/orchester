//! The person behind an account, past the account (N1 security re-review 2). The caps on what one account does (five
//! linked servers, 100 invites a day from the servers it linked) hold for the person who registered the servers, so
//! deleting the account and signing up again with the same email gives none back. A re-signup gets a new account id,
//! so the person is keyed by the account's email; the registry keeps that key, not the email: an HMAC-SHA256 of the
//! normalized email under a key derived from the key-encryption key, which nothing in the database can reverse or
//! check an email against. A server keeps its owner's key as `last_owner` when the owner deletes their account.

use deadpool_postgres::GenericClient;
use orchester_api::Credential;
use ring::{hkdf, hmac};
use uuid::Uuid;

use crate::{error::CloudError, keys::key_encryption_key};

/// Separates this key from the key-encryption key's other use, sealing the signing keys.
const INFO: &[u8] = b"orchester-cloud people v1";

/// Keys a person by the email of their account.
pub(crate) struct People(hmac::Key);

impl People {
  pub fn new(key: &Credential) -> Result<Self, CloudError> {
    let bytes = key_encryption_key(key)?;
    let extracted = hkdf::Salt::new(hkdf::HKDF_SHA256, &[]).extract(&bytes);
    let derived = extracted
      .expand(&[INFO], hmac::HMAC_SHA256)
      .map_err(|_| CloudError::Configuration("the people key cannot be derived"))?;
    Ok(Self(hmac::Key::from(derived)))
  }

  /// The person whose account has the normalized `email`: the same for every account with it.
  pub fn of(&self, email: &str) -> Vec<u8> {
    hmac::sign(&self.0, email.as_bytes()).as_ref().to_vec()
  }

  /// The person who owns `account`.
  pub async fn owning(&self, client: &impl GenericClient, account: Uuid) -> Result<Vec<u8>, CloudError> {
    let email: String = client
      .query_one("SELECT email FROM accounts WHERE id = $1", &[&account])
      .await?
      .try_get(0)?;
    Ok(self.of(&email))
  }
}

/// The bucket of `person`'s daily invites; `None` for a key this cloud did not make.
pub(crate) fn quota_key(person: &[u8]) -> Option<Uuid> {
  person.get(..16).and_then(|bytes| Uuid::from_slice(bytes).ok())
}
