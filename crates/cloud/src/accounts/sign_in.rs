use std::{net::IpAddr, time::SystemTime};

use deadpool_postgres::GenericClient;
use orchester_api::{
  Credential,
  cloud::{CloudDevice, CloudErrorCode, CloudSignIn, CloudSignInChallenge, CloudSignedIn, DeviceInfo, SessionEnd},
};
use orchester_core::id::{CloudAccountId, CloudDeviceId};

use super::{
  Cloud, input,
  rows::{ACTIVE, AccountRow, Authenticated, account_by_email, active_since},
  two_factor,
};
use crate::{
  clock::MINUTE_MS,
  db::{audit, millis, time},
  error::CloudError,
  passwords::PasswordHash,
  tokens::{DEVICE_PREFIX, random_token, token_hash, well_formed},
};

/// What the password step of a sign-in comes to.
pub(crate) enum SignInAnswer {
  SignedIn(Box<CloudSignedIn>),
  /// Two-factor authentication is on: a code finishes the sign-in (`POST /auth/sessions/two-factor`).
  Challenged(CloudSignInChallenge),
}

impl Cloud {
  /// A wrong password and an unknown email answer alike: the unknown one is verified against a dummy hash and counts
  /// against the same limits. `email_unconfirmed` comes only after the right password, and so does a challenge for an
  /// account with two-factor authentication. Answers the account it is for, for the request log.
  pub(crate) async fn sign_in(
    &self,
    input: CloudSignIn,
    address: IpAddr,
  ) -> Result<(CloudAccountId, SignInAnswer), CloudError> {
    let device = input::device(&input.device)?;
    let Ok(email) = input::email(&input.email) else {
      return Err(CloudErrorCode::InvalidCredentials.into());
    };
    let attempt = self.inner.limits.sign_in.sign_in(email.clone(), address, self.now())?;
    let stored = account_by_email(&self.inner.db.client().await?, &email).await?;
    let hash = stored.as_ref().map_or_else(PasswordHash::dummy, |row| row.hash.clone());
    let (verified, attempt) = self
      .inner
      .passwords
      .verify(input.password, hash.clone(), attempt, self.inner.clock.as_ref())
      .await?;
    let now = self.now();
    let account = match stored {
      Some(account) if verified => account,
      stored => {
        attempt.failed(now);
        self
          .record_failure(stored.map(|row| row.account.id), address, now)
          .await?;
        return Err(CloudErrorCode::InvalidCredentials.into());
      }
    };
    if !account.confirmed {
      attempt.succeeded();
      return Err(CloudErrorCode::EmailUnconfirmed.into());
    }
    let id = account.account.id;
    let answer = match two_factor::confirmed_id(&self.client().await?, id).await? {
      Some(two_factor) => self
        .challenge(&account, (two_factor, &hash), &device, address)
        .await?
        .map(SignInAnswer::Challenged),
      None => self
        .add_device(account, &hash, device, address)
        .await?
        .map(|signed_in| SignInAnswer::SignedIn(Box::new(signed_in))),
    };
    let Some(answer) = answer else {
      attempt.failed(now);
      self.record_failure(Some(id), address, now).await?;
      return Err(CloudErrorCode::InvalidCredentials.into());
    };
    attempt.succeeded();
    Ok((id, answer))
  }

  /// Adds the device, signed in without a second factor, only while the password is still the one just verified and
  /// two-factor authentication is still off; `None` otherwise. `FOR SHARE` waits for a reset or change that holds the
  /// account row and then checks its new hash, so a device can never be added after a reset signed every device out
  /// (review 1, M1).
  async fn add_device(
    &self,
    account: AccountRow,
    hash: &PasswordHash,
    device: DeviceInfo,
    address: IpAddr,
  ) -> Result<Option<CloudSignedIn>, CloudError> {
    let token = random_token(DEVICE_PREFIX)?;
    let id = CloudDeviceId::generate();
    let now = self.now();
    let client = self.inner.db.client().await?;
    let added = client
      .execute(
        "INSERT INTO devices (id, account, name, platform, token_hash, created_at, last_used_at)
         SELECT $1, id, $2, $3, $4, $5, $5 FROM accounts
         WHERE id = $6 AND password_hash = $7 AND confirmed_at IS NOT NULL
           AND NOT EXISTS (SELECT 1 FROM two_factor t WHERE t.account = accounts.id AND t.confirmed_at IS NOT NULL)
         FOR SHARE",
        &[
          &id.as_uuid(),
          &device.name,
          &device.platform.as_str(),
          &token_hash(&token),
          &time(now),
          &account.account.id.as_uuid(),
          &hash.expose(),
        ],
      )
      .await?;
    if added == 0 {
      return Ok(None);
    }
    audit(
      &client,
      Some(account.account.id),
      "signed_in",
      &id.to_string(),
      Some(address),
      now,
    )
    .await?;
    Ok(Some(signed_in(token, account, id, device, now)))
  }

  async fn record_failure(&self, account: Option<CloudAccountId>, address: IpAddr, now: i64) -> Result<(), CloudError> {
    let client = self.inner.db.client().await?;
    let target = account.map(|id| id.to_string()).unwrap_or_default();
    audit(&client, account, "sign_in_failed", &target, Some(address), now).await
  }

  /// The device of a bearer token. Its last use is written only when it moved by more than a minute.
  pub(crate) async fn authenticate(&self, token: &Credential) -> Result<Authenticated, CloudError> {
    if !well_formed(token.expose(), DEVICE_PREFIX) {
      return Err(CloudErrorCode::Unauthorized.into());
    }
    let now = self.now();
    let client = self.inner.db.client().await?;
    let (idle, web) = active_since(now);
    let hash = token_hash(token);
    let Some(row) = client
      .query_opt(
        &format!("SELECT id, account, last_used_at FROM devices WHERE token_hash = $1 AND {ACTIVE}"),
        &[&hash, &idle, &web],
      )
      .await?
    else {
      return Err(ended(&client, &hash).await?);
    };
    let auth = Authenticated {
      device: CloudDeviceId::from_uuid(row.try_get(0)?),
      account: CloudAccountId::from_uuid(row.try_get(1)?),
    };
    if now - millis(row.try_get::<_, SystemTime>(2)?)? > MINUTE_MS {
      client
        .execute(
          "UPDATE devices SET last_used_at = $1 WHERE id = $2 AND revoked_at IS NULL",
          &[&time(now), &auth.device.as_uuid()],
        )
        .await?;
    }
    Ok(auth)
  }
}

/// Why the token with `hash` no longer signs in: how its device was revoked, or that its account was deleted; plain
/// `unauthorized` for a token that ended unused, one revoked before reasons were kept, or one the cloud never issued.
async fn ended(client: &impl GenericClient, hash: &[u8]) -> Result<CloudError, CloudError> {
  let row = client
    .query_opt(
      "SELECT revoked_reason, revoked_at FROM devices WHERE token_hash = $1 AND revoked_reason IS NOT NULL
       UNION ALL SELECT reason, ended_at FROM ended_tokens WHERE token_hash = $1",
      &[&hash],
    )
    .await?;
  let Some(row) = row else {
    return Ok(CloudErrorCode::Unauthorized.into());
  };
  let reason = match row.try_get::<_, &str>(0)? {
    "revoked" => SessionEnd::Revoked,
    "password_reset" => SessionEnd::PasswordReset,
    "signed_out" => SessionEnd::SignedOut,
    "account_deleted" => SessionEnd::AccountDeleted,
    _ => return Err(CloudError::StoredData),
  };
  let at = millis(row.try_get::<_, SystemTime>(1)?)?;
  Ok(CloudError::Ended { reason, at })
}

pub(super) fn signed_in(
  token: Credential,
  account: AccountRow,
  id: CloudDeviceId,
  device: DeviceInfo,
  now: i64,
) -> CloudSignedIn {
  CloudSignedIn {
    token,
    device: CloudDevice {
      id,
      name: device.name,
      platform: device.platform,
      created_at: now,
      last_used_at: now,
      current: true,
    },
    account: account.account,
  }
}
