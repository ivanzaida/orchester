//! Two-factor authentication (`docs/cloud-api.md`, "Two-factor authentication"): one TOTP authenticator per account
//! (RFC 6238, `orchester_api::totp`), its secret sealed with the key-encryption key and bound to the account, and ten
//! one-time recovery codes kept as an HMAC. With it on, a sign-in's password step answers with a challenge that a code
//! finishes. A device that signed in, stepped up or turned it on with a code is a two-factor session
//! (`devices.two_factor`), and only such a session accepts invites. Turning it off deletes the authenticator, which
//! ends every two-factor session with it.

mod budget;
mod challenge;
mod codes;
mod setup;

pub(crate) use codes::RecoveryKey;
use deadpool_postgres::GenericClient;
use orchester_api::cloud::{CloudErrorCode, TwoFactorStatus};
use orchester_core::id::{CloudAccountId, CloudDeviceId};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{Authenticated, Cloud};
use crate::{
  db::{millis, time},
  error::CloudError,
};

/// An account's authenticator, its secret opened.
pub(super) struct Authenticator {
  id: Uuid,
  account: CloudAccountId,
  secret: Zeroizing<Vec<u8>>,
  confirmed: bool,
  created_at: i64,
}

/// What the sealed secret is bound to: the account it belongs to.
fn label(account: CloudAccountId) -> String {
  format!("two-factor {account}")
}

impl Cloud {
  /// The account's authenticator, confirmed or being set up, or `None`.
  async fn authenticator(
    &self,
    client: &impl GenericClient,
    account: CloudAccountId,
  ) -> Result<Option<Authenticator>, CloudError> {
    let Some(row) = client
      .query_opt(
        "SELECT id, sealed_secret, confirmed_at, created_at FROM two_factor WHERE account = $1",
        &[&account.as_uuid()],
      )
      .await?
    else {
      return Ok(None);
    };
    let secret = self.inner.sealing.open_bytes(&label(account), row.try_get(1)?)?;
    Ok(Some(Authenticator {
      id: row.try_get(0)?,
      account,
      secret,
      confirmed: row.try_get::<_, Option<std::time::SystemTime>>(2)?.is_some(),
      created_at: millis(row.try_get(3)?)?,
    }))
  }

  /// The account's confirmed authenticator; `None` while two-factor authentication is off.
  async fn confirmed_authenticator(
    &self,
    client: &impl GenericClient,
    account: CloudAccountId,
  ) -> Result<Option<Authenticator>, CloudError> {
    Ok(
      self
        .authenticator(client, account)
        .await?
        .filter(|authenticator| authenticator.confirmed),
    )
  }
}

/// The id of the account's confirmed authenticator, without opening its secret: whether a sign-in needs a code.
pub(super) async fn confirmed_id(
  client: &impl GenericClient,
  account: CloudAccountId,
) -> Result<Option<Uuid>, CloudError> {
  let row = client
    .query_opt(
      "SELECT id FROM two_factor WHERE account = $1 AND confirmed_at IS NOT NULL",
      &[&account.as_uuid()],
    )
    .await?;
  Ok(row.map(|row| row.try_get(0)).transpose()?)
}

/// The account's two-factor authentication as the asking device sees it (`GET /me`). `devices.two_factor` names only a
/// confirmed authenticator and is emptied when it is deleted, so a device that has one is a two-factor session.
pub(crate) async fn status(client: &impl GenericClient, auth: Authenticated) -> Result<TwoFactorStatus, CloudError> {
  let row = client
    .query_one(
      "SELECT t.confirmed_at,
         (SELECT count(*) FROM recovery_codes r WHERE r.two_factor = t.id AND r.used_at IS NULL),
         d.two_factor IS NOT NULL
       FROM devices d LEFT JOIN two_factor t ON t.account = d.account AND t.confirmed_at IS NOT NULL
       WHERE d.id = $1",
      &[&auth.device.as_uuid()],
    )
    .await?;
  let enabled_at = row
    .try_get::<_, Option<std::time::SystemTime>>(0)?
    .map(millis)
    .transpose()?;
  Ok(TwoFactorStatus {
    enabled: enabled_at.is_some(),
    enabled_at,
    recovery_codes_left: u32::try_from(row.try_get::<_, i64>(1)?).map_err(|_| CloudError::StoredData)?,
    this_session: row.try_get(2)?,
  })
}

/// Whether the device is a two-factor session, and whether its account has two-factor authentication on.
async fn standing(client: &impl GenericClient, device: CloudDeviceId) -> Result<(bool, bool), CloudError> {
  let row = client
    .query_one(
      "SELECT d.two_factor IS NOT NULL,
         EXISTS (SELECT 1 FROM two_factor t WHERE t.account = d.account AND t.confirmed_at IS NOT NULL)
       FROM devices d WHERE d.id = $1",
      &[&device.as_uuid()],
    )
    .await?;
  Ok((row.try_get(0)?, row.try_get(1)?))
}

/// Accepting an invite needs two-factor authentication on the account (`mfa_required`) and a device that signed in,
/// stepped up or turned it on with a code (`session_without_mfa`).
pub(crate) async fn require_session(client: &impl GenericClient, device: CloudDeviceId) -> Result<(), CloudError> {
  match standing(client, device).await? {
    (true, _) => Ok(()),
    (false, true) => Err(CloudErrorCode::SessionWithoutMfa.into()),
    (false, false) => Err(CloudErrorCode::MfaRequired.into()),
  }
}

/// While the account has two-factor authentication on, deleting it, changing its password or signing another of its
/// devices out needs a two-factor session (`session_without_mfa`): a password alone does not undo what the code
/// protects. With it off, any device of the account may.
pub(crate) async fn require_session_when_on(
  client: &impl GenericClient,
  device: CloudDeviceId,
) -> Result<(), CloudError> {
  match standing(client, device).await? {
    (false, true) => Err(CloudErrorCode::SessionWithoutMfa.into()),
    _ => Ok(()),
  }
}

/// Whether the device is a two-factor session: what an identity token's `mfa` claim says.
pub(crate) async fn session(client: &impl GenericClient, device: CloudDeviceId) -> Result<bool, CloudError> {
  Ok(standing(client, device).await?.0)
}

/// Setups not confirmed within this time end; the sweep deletes them.
pub(crate) const SETUP_MS: i64 = 15 * crate::clock::MINUTE_MS;

/// Deletes setups never confirmed and sign-in challenges past their time.
pub(crate) async fn sweep(client: &impl GenericClient, now: i64) -> Result<(), CloudError> {
  client
    .execute(
      "DELETE FROM two_factor WHERE confirmed_at IS NULL AND created_at <= $1",
      &[&time(now - SETUP_MS)],
    )
    .await?;
  client
    .execute("DELETE FROM sign_in_challenges WHERE expires_at <= $1", &[&time(now)])
    .await?;
  Ok(())
}
