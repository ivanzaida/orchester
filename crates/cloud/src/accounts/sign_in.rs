use std::{net::IpAddr, time::SystemTime};

use orchester_api::{
  Credential,
  cloud::{CloudDevice, CloudErrorCode, CloudSignIn, CloudSignedIn, DeviceInfo},
};
use orchester_core::id::{CloudAccountId, CloudDeviceId};

use super::{
  Cloud, input,
  rows::{ACTIVE, AccountRow, Authenticated, account_by_email, active_since},
};
use crate::{
  clock::MINUTE_MS,
  db::{audit, millis, time},
  error::CloudError,
  passwords::PasswordHash,
  tokens::{DEVICE_PREFIX, random_token, token_hash, well_formed},
};

impl Cloud {
  /// A wrong password and an unknown email answer alike: the unknown one is verified against a dummy hash and counts
  /// against the same limits. `email_unconfirmed` comes only after the right password.
  pub(crate) async fn sign_in(&self, input: CloudSignIn, address: IpAddr) -> Result<CloudSignedIn, CloudError> {
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
    let token = random_token(DEVICE_PREFIX)?;
    let id = CloudDeviceId::generate();
    let client = self.inner.db.client().await?;
    // Only while the password is still the one just verified: a change or reset meanwhile wins. `FOR SHARE` waits
    // for a reset or change that holds the account row and then checks its new hash, so a device can never be added
    // after a reset signed every device out (review 1, M1).
    let added = client
      .execute(
        "INSERT INTO devices (id, account, name, platform, token_hash, created_at, last_used_at)
         SELECT $1, id, $2, $3, $4, $5, $5 FROM accounts
         WHERE id = $6 AND password_hash = $7 AND confirmed_at IS NOT NULL FOR SHARE",
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
      attempt.failed(now);
      self.record_failure(Some(account.account.id), address, now).await?;
      return Err(CloudErrorCode::InvalidCredentials.into());
    }
    attempt.succeeded();
    audit(
      &client,
      Some(account.account.id),
      "signed_in",
      &id.to_string(),
      Some(address),
      now,
    )
    .await?;
    Ok(signed_in(token, account, id, device, now))
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
    let row = client
      .query_opt(
        &format!("SELECT id, account, last_used_at FROM devices WHERE token_hash = $1 AND {ACTIVE}"),
        &[&token_hash(token), &idle, &web],
      )
      .await?
      .ok_or(CloudErrorCode::Unauthorized)?;
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

fn signed_in(token: Credential, account: AccountRow, id: CloudDeviceId, device: DeviceInfo, now: i64) -> CloudSignedIn {
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
