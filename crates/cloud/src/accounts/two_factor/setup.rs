//! Turning two-factor authentication on and off, and new recovery codes. Turning it on takes the password, then a code
//! from the app that read the new secret; the recovery codes are shown once, and only the session that confirmed it
//! becomes a two-factor session. Turning it off takes the password and a code; new recovery codes take a code. Each
//! change is mailed to the account's email.

use std::net::IpAddr;

use deadpool_postgres::GenericClient;
use orchester_api::{
  Credential,
  cloud::{CloudErrorCode, RecoveryCodes, TwoFactorCode, TwoFactorOff, TwoFactorSetup, TwoFactorStart},
  totp,
};
use orchester_core::id::CloudAccountId;
use uuid::Uuid;

use super::{
  Authenticator, SETUP_MS,
  codes::{account_email, new_recovery_codes},
  label,
};
use crate::{
  Cloud,
  accounts::{
    Authenticated,
    rows::{AccountRow, account},
  },
  db::{audit, time},
  error::CloudError,
  mail::EmailPurpose,
  tokens::random_bytes,
};

impl Cloud {
  /// `POST /me/two-factor`: a new secret, replacing a setup not confirmed yet. `409 conflict` while it is on.
  pub(crate) async fn start_two_factor(
    &self,
    auth: Authenticated,
    input: TwoFactorStart,
    address: IpAddr,
  ) -> Result<TwoFactorSetup, CloudError> {
    let row = self.confirm_password(auth, input.password, address).await?;
    let secret = zeroize::Zeroizing::new(random_bytes::<20>()?);
    let sealed = self.inner.sealing.seal_bytes(&label(auth.account), secret.as_slice())?;
    let now = self.now();
    let mut client = self.client().await?;
    let transaction = client.transaction().await?;
    transaction
      .execute(
        "DELETE FROM two_factor WHERE account = $1 AND confirmed_at IS NULL",
        &[&auth.account.as_uuid()],
      )
      .await?;
    let added = transaction
      .execute(
        "INSERT INTO two_factor (id, account, sealed_secret, created_at) VALUES ($1, $2, $3, $4)
         ON CONFLICT (account) DO NOTHING",
        &[&Uuid::now_v7(), &auth.account.as_uuid(), &sealed, &time(now)],
      )
      .await?;
    if added == 0 {
      return Err(CloudErrorCode::Conflict.into());
    }
    audit(
      &transaction,
      Some(auth.account),
      "two_factor_setup_started",
      &auth.device.to_string(),
      Some(address),
      now,
    )
    .await?;
    transaction.commit().await?;
    let encoded = totp::encode_secret(secret.as_slice());
    let uri = self.otpauth(&row.account.email, &encoded);
    Ok(TwoFactorSetup {
      secret: Credential::new(encoded),
      uri: Credential::new(uri),
    })
  }

  /// `POST /me/two-factor/confirm`: a code from the app that read the secret turns it on, makes this device a
  /// two-factor session and answers the ten recovery codes, once. `404` without a setup in its 15 minutes.
  pub(crate) async fn confirm_two_factor(
    &self,
    auth: Authenticated,
    input: TwoFactorCode,
  ) -> Result<RecoveryCodes, CloudError> {
    let mut client = self.client().await?;
    let pending = self
      .authenticator(&client, auth.account)
      .await?
      .filter(|authenticator| !authenticator.confirmed && authenticator.created_at > self.now() - SETUP_MS)
      .ok_or(CloudErrorCode::NotFound)?;
    self
      .check_code(&client, (&pending, auth.device), &input.code, false)
      .await?;
    let codes = new_recovery_codes()?;
    let now = self.now();
    let transaction = client.transaction().await?;
    let confirmed = transaction
      .execute(
        "UPDATE two_factor SET confirmed_at = $2 WHERE id = $1 AND confirmed_at IS NULL",
        &[&pending.id, &time(now)],
      )
      .await?;
    if confirmed == 0 {
      return Err(CloudErrorCode::NotFound.into());
    }
    self.store_recovery_codes(&transaction, &pending, &codes).await?;
    transaction
      .execute(
        "UPDATE devices SET two_factor = $1 WHERE id = $2",
        &[&pending.id, &auth.device.as_uuid()],
      )
      .await?;
    audit(
      &transaction,
      Some(auth.account),
      "two_factor_enabled",
      &auth.device.to_string(),
      None,
      now,
    )
    .await?;
    let email = account_email(&transaction, auth.account).await?;
    transaction.commit().await?;
    self.send_later(self.inner.messages.two_factor(email, EmailPurpose::TwoFactorOn));
    Ok(recovery_codes(codes))
  }

  /// `DELETE /me/two-factor`: the password and a code turn it off. Its recovery codes go, and every device stops being
  /// a two-factor session. `404` while it is off.
  pub(crate) async fn turn_off_two_factor(
    &self,
    auth: Authenticated,
    input: TwoFactorOff,
    address: IpAddr,
  ) -> Result<(), CloudError> {
    let row = self.confirm_password(auth, input.password, address).await?;
    let client = self.client().await?;
    let authenticator = self.confirmed(&client, auth.account).await?;
    self
      .check_code(&client, (&authenticator, auth.device), &input.code, true)
      .await?;
    client
      .execute("DELETE FROM two_factor WHERE id = $1", &[&authenticator.id])
      .await?;
    audit(
      &client,
      Some(auth.account),
      "two_factor_disabled",
      &auth.device.to_string(),
      Some(address),
      self.now(),
    )
    .await?;
    self.send_later(
      self
        .inner
        .messages
        .two_factor(row.account.email, EmailPurpose::TwoFactorOff),
    );
    Ok(())
  }

  /// `POST /me/two-factor/recovery-codes`: a code replaces every recovery code with ten new ones, shown once.
  pub(crate) async fn new_recovery_codes(
    &self,
    auth: Authenticated,
    input: TwoFactorCode,
  ) -> Result<RecoveryCodes, CloudError> {
    let mut client = self.client().await?;
    let authenticator = self.confirmed(&client, auth.account).await?;
    self
      .check_code(&client, (&authenticator, auth.device), &input.code, true)
      .await?;
    let codes = new_recovery_codes()?;
    let transaction = client.transaction().await?;
    transaction
      .execute("DELETE FROM recovery_codes WHERE two_factor = $1", &[&authenticator.id])
      .await?;
    self.store_recovery_codes(&transaction, &authenticator, &codes).await?;
    audit(
      &transaction,
      Some(auth.account),
      "recovery_codes_made",
      &auth.device.to_string(),
      None,
      self.now(),
    )
    .await?;
    transaction.commit().await?;
    Ok(recovery_codes(codes))
  }

  /// The confirmed authenticator; `404` while two-factor authentication is off.
  async fn confirmed(&self, client: &impl GenericClient, account: CloudAccountId) -> Result<Authenticator, CloudError> {
    self
      .confirmed_authenticator(client, account)
      .await?
      .ok_or_else(|| CloudErrorCode::NotFound.into())
  }

  /// The account's password, asked again; wrong ones count as a password change's do.
  async fn confirm_password(
    &self,
    auth: Authenticated,
    password: Credential,
    address: IpAddr,
  ) -> Result<AccountRow, CloudError> {
    let attempt = self.inner.limits.sign_in.password(auth.account, self.now())?;
    let row = account(&self.client().await?, auth.account).await?;
    let (verified, attempt) = self
      .inner
      .passwords
      .verify(password, row.hash.clone(), attempt, self.inner.clock.as_ref())
      .await?;
    if verified {
      attempt.succeeded();
      return Ok(row);
    }
    let now = self.now();
    attempt.failed(now);
    let client = self.client().await?;
    audit(
      &client,
      Some(auth.account),
      "two_factor_password_failed",
      &auth.device.to_string(),
      Some(address),
      now,
    )
    .await?;
    Err(CloudErrorCode::InvalidCredentials.into())
  }

  /// The link an authenticator app reads from the QR code: the cloud's host names the account in the app, so a test
  /// cloud's entry is not mistaken for the person's real one.
  fn otpauth(&self, email: &str, secret: &str) -> String {
    let url = &self.inner.config.public_url;
    let host = url.split_once("://").map_or(url.as_str(), |(_, rest)| rest);
    let issuer = encode(host.split('/').next().unwrap_or(host));
    format!(
      "otpauth://totp/{issuer}:{email}?secret={secret}&issuer={issuer}&algorithm=SHA1&digits={digits}&period={period}",
      email = encode(email),
      digits = totp::DIGITS,
      period = totp::STEP_SECS,
    )
  }
}

fn recovery_codes(codes: Vec<String>) -> RecoveryCodes {
  RecoveryCodes {
    recovery_codes: codes.into_iter().map(Credential::new).collect(),
  }
}

/// Percent-encodes everything but RFC 3986's unreserved characters.
fn encode(text: &str) -> String {
  let mut encoded = String::with_capacity(text.len());
  for byte in text.bytes() {
    if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
      encoded.push(char::from(byte));
    } else {
      encoded.push_str(&format!("%{byte:02X}"));
    }
  }
  encoded
}
