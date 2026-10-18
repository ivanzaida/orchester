//! How many two-factor codes an account may try (`docs/cloud-api.md`, "Two-factor authentication"): five a quarter
//! hour, kept in memory (`Limits::two_factor_code`), and twenty wrong ones a day, kept in the database so a restart or
//! another replica gives a guesser no more. The day's count is the authenticator's for sign-ins and two-factor
//! sessions, and a device's own for a device that is not one, so a device signed in without a code cannot lock its
//! owner out of signing in. The tenth wrong code of a day mails the account's owner, once.

use deadpool_postgres::GenericClient;
use orchester_api::cloud::CloudErrorCode;
use orchester_core::id::CloudDeviceId;
use uuid::Uuid;

use super::{Authenticator, codes::Used};
use crate::{
  Cloud,
  clock::MINUTE_MS,
  db::{audit, millis, time},
  error::CloudError,
  mail::EmailPurpose,
};

/// Wrong codes an account may enter in a day, counted from the first of them.
const WRONG_A_DAY: i32 = 20;
/// The wrong code of a day that mails the owner.
const MAIL_AT: i32 = 10;
const DAY_MS: i64 = 24 * 60 * MINUTE_MS;

/// Whose day a wrong code counts in.
#[derive(Clone, Copy)]
pub(super) enum Tally {
  /// The authenticator's: a sign-in's challenge, or a device that is a two-factor session.
  Account,
  /// A device's own, while it is not a two-factor session.
  Device(CloudDeviceId),
}

impl Tally {
  /// The tally of a code entered on `device`.
  pub(super) async fn of(client: &impl GenericClient, device: CloudDeviceId) -> Result<Self, CloudError> {
    Ok(if super::session(client, device).await? {
      Self::Account
    } else {
      Self::Device(device)
    })
  }

  /// The table that keeps the count, and the row's id there.
  fn row(self, authenticator: &Authenticator) -> (&'static str, Uuid) {
    match self {
      Self::Account => ("two_factor", authenticator.id),
      Self::Device(device) => ("devices", device.as_uuid()),
    }
  }
}

/// A code check the budget allowed: its quarter-hour reservation, which a right code gives back, and its tally.
pub(super) struct Attempt {
  reserved_at: i64,
  tally: Tally,
}

impl Cloud {
  /// One more code for `authenticator`, if the day of `tally` has wrong codes to spare (`429 rate_limited` until the
  /// day ends otherwise) and the account's quarter hour does too.
  pub(super) async fn attempt_code(
    &self,
    client: &impl GenericClient,
    authenticator: &Authenticator,
    tally: Tally,
  ) -> Result<Attempt, CloudError> {
    let now = self.now();
    let (table, id) = tally.row(authenticator);
    let row = client
      .query_one(
        &format!("SELECT failed_codes, failures_since FROM {table} WHERE id = $1"),
        &[&id],
      )
      .await?;
    let failed: i32 = row.try_get(0)?;
    let since = row
      .try_get::<_, Option<std::time::SystemTime>>(1)?
      .map(millis)
      .transpose()?;
    if let Some(since) = since
      && since + DAY_MS > now
      && failed >= WRONG_A_DAY
    {
      let seconds = (since + DAY_MS - now + 999) / 1000;
      return Err(CloudError::RateLimited(seconds.unsigned_abs()));
    }
    let reserved_at = self.inner.limits.two_factor_code(authenticator.account, now)?;
    Ok(Attempt { reserved_at, tally })
  }

  /// A right code: its reservation back, and a used recovery code audited and mailed to the owner.
  pub(super) async fn code_passed(
    &self,
    client: &impl GenericClient,
    authenticator: &Authenticator,
    (attempt, used): (Attempt, Used),
  ) -> Result<(), CloudError> {
    let account = authenticator.account;
    self.inner.limits.two_factor_passed(account, attempt.reserved_at);
    if used == Used::Recovery {
      audit(
        client,
        Some(account),
        "recovery_code_used",
        &account.to_string(),
        None,
        self.now(),
      )
      .await?;
      let email = super::codes::account_email(client, account).await?;
      self.send_later(self.inner.messages.two_factor(email, EmailPurpose::RecoveryCodeUsed));
    }
    Ok(())
  }

  /// A wrong or used code: audited and counted in the day of its attempt's tally, with the owner mailed at the day's
  /// tenth. Answers the `invalid_code` to give.
  pub(super) async fn code_failed(
    &self,
    client: &impl GenericClient,
    authenticator: &Authenticator,
    attempt: &Attempt,
  ) -> Result<CloudError, CloudError> {
    let account = authenticator.account;
    let now = self.now();
    audit(
      client,
      Some(account),
      "two_factor_failed",
      &account.to_string(),
      None,
      now,
    )
    .await?;
    // An authenticator turned off meanwhile counts nothing more.
    let (table, id) = attempt.tally.row(authenticator);
    let row = client
      .query_opt(
        &format!(
          "UPDATE {table} SET
             failed_codes = CASE WHEN failures_since IS NULL OR failures_since <= $3 THEN 1 ELSE failed_codes + 1 END,
             failures_since = CASE WHEN failures_since IS NULL OR failures_since <= $3 THEN $2 ELSE failures_since END
           WHERE id = $1 RETURNING failed_codes"
        ),
        &[&id, &time(now), &time(now - DAY_MS)],
      )
      .await?;
    if row.map(|row| row.try_get::<_, i32>(0)).transpose()? == Some(MAIL_AT) {
      let email = super::codes::account_email(client, account).await?;
      self.send_later(self.inner.messages.two_factor(email, EmailPurpose::TwoFactorFailures));
    }
    Ok(CloudErrorCode::InvalidCode.into())
  }
}
