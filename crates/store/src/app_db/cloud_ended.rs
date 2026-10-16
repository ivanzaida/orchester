//! Why this computer's last Orchester Cloud sign-in ended (the sign-in gate, 2026-10-16): at most one row, the email
//! that was signed in, the reason and when. The gate names the reason in its notice and fills in the email; the row
//! goes when an account signs in again.

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
      Self::Other => "other",
    }
  }

  fn parse(text: &str) -> Result<Self, StoreError> {
    Ok(match text {
      "revoked" => Self::Revoked,
      "password_reset" => Self::PasswordReset,
      "account_deleted" => Self::AccountDeleted,
      "signed_out" => Self::SignedOut,
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
}

impl AppDb {
  pub fn cloud_sign_in_ended(&self) -> Result<Option<EndedCloudSignIn>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT email, reason, ended_at FROM cloud_sign_in_ended WHERE id = 1",
        [],
        |row| {
          Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
          ))
        },
      )
      .optional()?;
    row
      .map(|(email, reason, ended_at)| {
        Ok(EndedCloudSignIn {
          email,
          reason: CloudSignInEnd::parse(&reason)?,
          ended_at: Timestamp::from_unix_millis(ended_at),
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
        "INSERT INTO cloud_sign_in_ended (id, email, reason, ended_at) VALUES (1, ?1, ?2, ?3)",
        params![ended.email, ended.reason.as_str(), ended.ended_at.unix_millis()],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }
}
