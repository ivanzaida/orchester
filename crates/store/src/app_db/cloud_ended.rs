//! Why this computer's last Orchester Cloud sign-in ended (2026-10-16): at most one row, the email that was signed in,
//! the reason and when, and for a sign-in ended to sign in again with a code, the invite that asked for it. The sign-in
//! names the reason in its notice and fills in the email; the row goes when an account signs in again.

use orchester_core::time::Timestamp;
use rusqlite::{OptionalExtension, params};

use super::AppDb;
use crate::{StoreError, rows::corrupt};

pub(super) const MIGRATION: &str = r"
CREATE TABLE cloud_sign_in_ended (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  email TEXT NOT NULL,
  reason TEXT NOT NULL CHECK (reason IN ('revoked', 'password_reset', 'account_deleted', 'signed_out', 'other')),
  ended_at INTEGER NOT NULL
) STRICT;
";

/// Two-factor authentication (2026-10-17): a sign-in ended here so the person signs in again with a code, which
/// accepting an invite needs; `invite` names what the invite was to.
pub(super) const TWO_FACTOR_MIGRATION: &str = r"
CREATE TABLE cloud_sign_in_ended_two_factor (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  email TEXT NOT NULL,
  reason TEXT NOT NULL
    CHECK (reason IN ('revoked', 'password_reset', 'account_deleted', 'signed_out', 'two_factor', 'other')),
  ended_at INTEGER NOT NULL,
  invite TEXT
) STRICT;
INSERT INTO cloud_sign_in_ended_two_factor (id, email, reason, ended_at)
  SELECT id, email, reason, ended_at FROM cloud_sign_in_ended;
DROP TABLE cloud_sign_in_ended;
ALTER TABLE cloud_sign_in_ended_two_factor RENAME TO cloud_sign_in_ended;
";

/// Why a sign-in ended, as the cloud's `401` or this computer said it (`docs/cloud-api.md`, "Why a token ended").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloudSignInEnd {
  /// Signed out from another device, or from Devices on the cloud's account page.
  Revoked,
  /// The password was changed or reset.
  PasswordReset,
  AccountDeleted,
  /// Signed out on this computer.
  SignedOut,
  /// Signed out on this computer to sign in again with a two-factor code, which accepting an invite needs.
  TwoFactor,
  /// The cloud refused the sign-in and gave no reason this app knows.
  Other,
}

impl CloudSignInEnd {
  const fn as_str(self) -> &'static str {
    match self {
      Self::Revoked => "revoked",
      Self::PasswordReset => "password_reset",
      Self::AccountDeleted => "account_deleted",
      Self::SignedOut => "signed_out",
      Self::TwoFactor => "two_factor",
      Self::Other => "other",
    }
  }

  fn parse(text: &str) -> Result<Self, StoreError> {
    Ok(match text {
      "revoked" => Self::Revoked,
      "password_reset" => Self::PasswordReset,
      "account_deleted" => Self::AccountDeleted,
      "signed_out" => Self::SignedOut,
      "two_factor" => Self::TwoFactor,
      "other" => Self::Other,
      other => return Err(corrupt("cloud sign-in end", other)),
    })
  }
}

/// The last sign-in from this computer that ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndedCloudSignIn {
  pub email: String,
  pub reason: CloudSignInEnd,
  pub ended_at: Timestamp,
  /// What the invite that asked for a sign-in with a code was to ([`CloudSignInEnd::TwoFactor`]).
  pub invite: Option<String>,
}

impl AppDb {
  pub fn cloud_sign_in_ended(&self) -> Result<Option<EndedCloudSignIn>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT email, reason, ended_at, invite FROM cloud_sign_in_ended WHERE id = 1",
        [],
        |row| {
          Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, Option<String>>(3)?,
          ))
        },
      )
      .optional()?;
    row
      .map(|(email, reason, ended_at, invite)| {
        Ok(EndedCloudSignIn {
          email,
          reason: CloudSignInEnd::parse(&reason)?,
          ended_at: Timestamp::from_unix_millis(ended_at),
          invite,
        })
      })
      .transpose()
  }

  /// Records why the sign-in ended, in place of an earlier one; `None` once an account signed in again.
  pub fn set_cloud_sign_in_ended(&mut self, ended: Option<&EndedCloudSignIn>) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute("DELETE FROM cloud_sign_in_ended", [])?;
    if let Some(ended) = ended {
      transaction.execute(
        "INSERT INTO cloud_sign_in_ended (id, email, reason, ended_at, invite) VALUES (1, ?1, ?2, ?3, ?4)",
        params![
          ended.email,
          ended.reason.as_str(),
          ended.ended_at.unix_millis(),
          ended.invite
        ],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }
}
