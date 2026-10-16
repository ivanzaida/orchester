//! This computer's Orchester Cloud account (`docs/architecture/server.md`, "Desktop data", row 41): at most one row,
//! the account signed in from this computer and the cloud's address it was signed in at; and the cloud address the
//! person set in Settings › Orchester's advanced field, for a test cloud. The cloud device token is in the OS
//! keychain under this installation's id (`VaultKey::cloud_device`), never here.

use orchester_core::{id::CloudAccountId, time::Timestamp};
use rusqlite::{OptionalExtension, params};

use super::AppDb;
use crate::{StoreError, rows::parse};

pub(super) const MIGRATION: &str = r"
CREATE TABLE cloud_account (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  account_id TEXT NOT NULL,
  email TEXT NOT NULL,
  display_name TEXT NOT NULL,
  device_name TEXT NOT NULL,
  signed_in_at INTEGER NOT NULL,
  cloud_address TEXT NOT NULL
) STRICT;
-- Settings › Orchester › Advanced: the cloud this computer uses; NULL for the build's own.
ALTER TABLE app_settings ADD COLUMN cloud_address TEXT;
-- How this computer joined each team server: with a code, or with its Orchester Cloud identity for the server the
-- cloud knows by `team_server`.
ALTER TABLE servers ADD COLUMN joined_with TEXT CHECK (joined_with IN ('code', 'cloud'));
ALTER TABLE servers ADD COLUMN team_server TEXT
  CHECK ((team_server IS NOT NULL) = (joined_with IS NOT NULL AND joined_with = 'cloud'));
";

/// The Orchester Cloud account signed in from this computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredCloudAccount {
  pub account: CloudAccountId,
  pub email: String,
  pub display_name: String,
  /// How the cloud lists this computer among the account's devices.
  pub device_name: String,
  pub signed_in_at: Timestamp,
  /// The cloud's address at sign-in, e.g. `https://orchester-cloud.lurq.dev`.
  pub cloud_address: String,
}

impl AppDb {
  /// The cloud account signed in from this computer, if any.
  pub fn cloud_account(&self) -> Result<Option<StoredCloudAccount>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT account_id, email, display_name, device_name, signed_in_at, cloud_address FROM cloud_account \
         WHERE id = 1",
        [],
        |row| {
          Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
          ))
        },
      )
      .optional()?;
    row
      .map(
        |(account, email, display_name, device_name, signed_in_at, cloud_address)| {
          Ok(StoredCloudAccount {
            account: parse("cloud account id", &account)?,
            email,
            display_name,
            device_name,
            signed_in_at: Timestamp::from_unix_millis(signed_in_at),
            cloud_address,
          })
        },
      )
      .transpose()
  }

  /// Records the account signed in from this computer, replacing any other; `None` after signing out.
  pub fn set_cloud_account(&mut self, account: Option<&StoredCloudAccount>) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute("DELETE FROM cloud_account", [])?;
    if let Some(account) = account {
      transaction.execute(
        "INSERT INTO cloud_account (id, account_id, email, display_name, device_name, signed_in_at, cloud_address) \
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6)",
        params![
          account.account.to_string(),
          account.email,
          account.display_name,
          account.device_name,
          account.signed_in_at.unix_millis(),
          account.cloud_address,
        ],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }

  /// The cloud address set in Settings › Orchester's advanced field; `None` for the build's own.
  pub fn cloud_address(&self) -> Result<Option<String>, StoreError> {
    Ok(
      self
        .connection
        .query_row("SELECT cloud_address FROM app_settings WHERE id = 1", [], |row| {
          row.get(0)
        })?,
    )
  }

  pub fn set_cloud_address(&mut self, address: Option<&str>) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE app_settings SET cloud_address = ?1 WHERE id = 1",
      params![address],
    )?;
    Ok(())
  }
}
