//! The second step of a sign-in, and stepping a signed-in device up. With two-factor authentication on, the right
//! password answers a challenge instead of a device token: it works once, for five minutes and five codes, and only
//! while the password is still the one checked. A code then adds the device as a two-factor session. A device signed in
//! without a code (before it was turned on, or elsewhere) becomes one with a code alone, keeping its token.

use std::net::IpAddr;

use deadpool_postgres::GenericClient;
use orchester_api::{
  Credential,
  cloud::{
    CloudErrorCode, CloudPlatform, CloudSignInChallenge, DeviceInfo, SteppedUp, TwoFactorCode, TwoFactorSignIn,
    TwoFactorSignedIn,
  },
};
use orchester_core::id::{CloudAccountId, CloudDeviceId};
use tokio_postgres::Row;
use uuid::Uuid;

use super::{Authenticator, codes::recovery_codes_left};
use crate::{
  Cloud,
  accounts::{
    Authenticated,
    rows::{AccountRow, account},
    sign_in::signed_in,
  },
  clock::MINUTE_MS,
  db::{audit, time},
  error::CloudError,
  passwords::PasswordHash,
  tokens::{random_token, token_hash, well_formed},
};

/// A challenge: 32 random bytes as `orcm_` and 64 hex characters, like a device token but never one.
const CHALLENGE_PREFIX: &str = "orcm_";
const CHALLENGE_MS: i64 = 5 * MINUTE_MS;
const CHALLENGE_TRIES: i32 = 5;

/// A challenge taken for one more try: what its sign-in will add.
struct Pending {
  account: CloudAccountId,
  two_factor: Uuid,
  device: DeviceInfo,
}

impl Pending {
  fn of(row: &Row) -> Result<Self, CloudError> {
    Ok(Self {
      account: CloudAccountId::from_uuid(row.try_get(0)?),
      two_factor: row.try_get(1)?,
      device: DeviceInfo {
        name: row.try_get(2)?,
        platform: CloudPlatform::parse(row.try_get(3)?).ok_or(CloudError::StoredData)?,
      },
    })
  }
}

impl Cloud {
  /// The challenge of a sign-in whose password was right, for the account's authenticator `two_factor`; `None` when
  /// the password changed since it was checked (a reset or a change meanwhile wins, as for a password-only sign-in).
  pub(in crate::accounts) async fn challenge(
    &self,
    row: &AccountRow,
    (two_factor, checked): (Uuid, &PasswordHash),
    device: &DeviceInfo,
    address: IpAddr,
  ) -> Result<Option<CloudSignInChallenge>, CloudError> {
    let token = random_token(CHALLENGE_PREFIX)?;
    let now = self.now();
    let client = self.client().await?;
    let added = client
      .execute(
        "INSERT INTO sign_in_challenges (token_hash, account, two_factor, device_name, platform, password_check,
           created_at, expires_at)
         SELECT $1, id, $2, $3, $4, sha256(convert_to(password_hash, 'UTF8')), $5, $6 FROM accounts
         WHERE id = $7 AND password_hash = $8 AND confirmed_at IS NOT NULL FOR SHARE",
        &[
          &token_hash(&token),
          &two_factor,
          &device.name,
          &device.platform.as_str(),
          &time(now),
          &time(now + CHALLENGE_MS),
          &row.account.id.as_uuid(),
          &checked.expose(),
        ],
      )
      .await?;
    if added == 0 {
      return Ok(None);
    }
    let target = row.account.id.to_string();
    audit(
      &client,
      Some(row.account.id),
      "sign_in_challenged",
      &target,
      Some(address),
      now,
    )
    .await?;
    Ok(Some(CloudSignInChallenge {
      challenge: token,
      expires_at: now + CHALLENGE_MS,
    }))
  }

  /// `POST /auth/sessions/two-factor`: the challenge and a code sign the device in as a two-factor session. A challenge
  /// that is unknown, used, expired or out of tries, or whose password or authenticator changed, is
  /// `challenge_expired`.
  pub(crate) async fn finish_sign_in(
    &self,
    input: TwoFactorSignIn,
    address: IpAddr,
  ) -> Result<TwoFactorSignedIn, CloudError> {
    let expired = || CloudError::from(CloudErrorCode::ChallengeExpired);
    if !well_formed(input.challenge.expose(), CHALLENGE_PREFIX) {
      return Err(expired());
    }
    let hash = token_hash(&input.challenge);
    let mut client = self.client().await?;
    let pending = self.try_challenge(&client, &hash).await?.ok_or_else(expired)?;
    let authenticator = self
      .confirmed_authenticator(&client, pending.account)
      .await?
      .filter(|authenticator| authenticator.id == pending.two_factor)
      .ok_or_else(expired)?;
    let attempt = self.attempt_code(&client, &authenticator).await?;
    let token = random_token(crate::tokens::DEVICE_PREFIX)?;
    let id = CloudDeviceId::generate();
    let now = self.now();
    let transaction = client.transaction().await?;
    // The challenge is taken before the code is spent, in one transaction: another request with it waits for this
    // one, then finds it used and spends no code of its own; a wrong code rolls the taking back.
    let taken = transaction
      .execute(
        "UPDATE sign_in_challenges SET used_at = $2 WHERE token_hash = $1 AND used_at IS NULL",
        &[&hash, &time(now)],
      )
      .await?;
    if taken == 0 {
      return Err(expired());
    }
    let Some(used) = self.spend_code(&transaction, &authenticator, &input.code, true).await? else {
      transaction.rollback().await?;
      return Err(self.code_failed(&client, &authenticator).await?);
    };
    if !add_device(&transaction, (&pending, &hash), (id, &token), now).await? {
      return Err(expired());
    }
    audit(
      &transaction,
      Some(pending.account),
      "signed_in",
      &id.to_string(),
      Some(address),
      now,
    )
    .await?;
    let row = account(&transaction, pending.account).await?;
    let recovery_codes_left = recovery_codes_left(&transaction, &authenticator).await?;
    transaction.commit().await?;
    self.code_passed(&client, &authenticator, (attempt, used)).await?;
    Ok(TwoFactorSignedIn {
      signed_in: signed_in(token, row, id, pending.device, now),
      recovery_codes_left,
    })
  }

  /// One more try of the challenge `hash`, if it has one left, is live and its password did not change since.
  async fn try_challenge(&self, client: &impl GenericClient, hash: &[u8]) -> Result<Option<Pending>, CloudError> {
    let row = client
      .query_opt(
        "UPDATE sign_in_challenges c SET attempts = attempts + 1
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > $2 AND attempts < $3
           AND password_check = (SELECT sha256(convert_to(a.password_hash, 'UTF8')) FROM accounts a WHERE a.id = c.account)
         RETURNING account, two_factor, device_name, platform",
        &[&hash, &time(self.now()), &CHALLENGE_TRIES],
      )
      .await?;
    row.as_ref().map(Pending::of).transpose()
  }

  /// `POST /auth/sessions/current/two-factor`: a code makes the asking device a two-factor session, with the same
  /// token. `403 mfa_required` while two-factor authentication is off.
  pub(crate) async fn step_up(&self, auth: Authenticated, input: TwoFactorCode) -> Result<SteppedUp, CloudError> {
    let client = self.client().await?;
    let authenticator: Authenticator = self
      .confirmed_authenticator(&client, auth.account)
      .await?
      .ok_or(CloudErrorCode::MfaRequired)?;
    self.check_code(&client, &authenticator, &input.code, true).await?;
    client
      .execute(
        "UPDATE devices SET two_factor = $1 WHERE id = $2 AND revoked_at IS NULL",
        &[&authenticator.id, &auth.device.as_uuid()],
      )
      .await?;
    audit(
      &client,
      Some(auth.account),
      "two_factor_stepped_up",
      &auth.device.to_string(),
      None,
      self.now(),
    )
    .await?;
    Ok(SteppedUp {
      recovery_codes_left: recovery_codes_left(&client, &authenticator).await?,
    })
  }
}

/// Adds the device of `pending`, the challenge `challenge`'s, as a two-factor session, only while the account's
/// password and authenticator are the ones the challenge was made for. `FOR SHARE` waits for a reset or a change
/// holding the account row, as a password-only sign-in does.
async fn add_device(
  client: &impl GenericClient,
  (pending, challenge): (&Pending, &[u8]),
  (id, token): (CloudDeviceId, &Credential),
  now: i64,
) -> Result<bool, CloudError> {
  let added = client
    .execute(
      "INSERT INTO devices (id, account, name, platform, token_hash, created_at, last_used_at, two_factor)
       SELECT $1, a.id, $2, $3, $4, $5, $5, t.id
       FROM accounts a JOIN two_factor t ON t.account = a.id JOIN sign_in_challenges c ON c.account = a.id
       WHERE a.id = $6 AND c.token_hash = $7 AND c.password_check = sha256(convert_to(a.password_hash, 'UTF8'))
         AND a.confirmed_at IS NOT NULL AND t.id = $8 AND t.confirmed_at IS NOT NULL
       FOR SHARE OF a, t",
      &[
        &id.as_uuid(),
        &pending.device.name,
        &pending.device.platform.as_str(),
        &token_hash(token),
        &time(now),
        &pending.account.as_uuid(),
        &challenge,
        &pending.two_factor,
      ],
    )
    .await?;
  Ok(added == 1)
}
