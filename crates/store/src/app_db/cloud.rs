//! This computer's Orchester Cloud account (`docs/architecture/server.md`, "Desktop data", row 41): at most one row,
//! the account signed in from this computer and the cloud's addresses it was signed in at; and the cloud addresses the
//! person set in Settings › Orchester Cloud's advanced field, for a test or self-run cloud. A cloud has a web address
//! and, when its API has a host of its own, an API address (`docs/cloud-api.md`, "Base"). The cloud device token is in
//! the OS keychain under this installation's id (`VaultKey::cloud_device`), never here.

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

pub(super) const API_ADDRESS_MIGRATION: &str = r"
-- The cloud's API address, when its API has a host of its own (api.orchester.cloud); NULL: the API is at /api/v1 on
-- the web address.
ALTER TABLE cloud_account ADD COLUMN cloud_api_address TEXT;
ALTER TABLE app_settings ADD COLUMN cloud_api_address TEXT;
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
  /// The cloud's web address at sign-in, e.g. `https://orchester.cloud`.
  pub cloud_address: String,
  /// The cloud's API address at sign-in, e.g. `https://api.orchester.cloud`; `None` for a cloud whose API is on its
  /// web address.
  pub cloud_api_address: Option<String>,
}

/// The cloud set in Settings › Orchester Cloud's advanced field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredCloudAddress {
  pub web: String,
  /// `None`: the cloud's API is on its web address.
  pub api: Option<String>,
}

impl AppDb {
  /// The cloud account signed in from this computer, if any.
  pub fn cloud_account(&self) -> Result<Option<StoredCloudAccount>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT account_id, email, display_name, device_name, signed_in_at, cloud_address, cloud_api_address \
         FROM cloud_account WHERE id = 1",
        [],
        |row| {
          Ok((
            (
              row.get::<_, String>(0)?,
              row.get::<_, String>(1)?,
              row.get::<_, String>(2)?,
              row.get::<_, String>(3)?,
            ),
            row.get::<_, i64>(4)?,
            (row.get::<_, String>(5)?, row.get::<_, Option<String>>(6)?),
          ))
        },
      )
      .optional()?;
    row
      .map(
        |((account, email, display_name, device_name), signed_in_at, (cloud_address, cloud_api_address))| {
          Ok(StoredCloudAccount {
            account: parse("cloud account id", &account)?,
            email,
            display_name,
            device_name,
            signed_in_at: Timestamp::from_unix_millis(signed_in_at),
            cloud_address,
            cloud_api_address,
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
        "INSERT INTO cloud_account (id, account_id, email, display_name, device_name, signed_in_at, cloud_address, \
         cloud_api_address) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
          account.account.to_string(),
          account.email,
          account.display_name,
          account.device_name,
          account.signed_in_at.unix_millis(),
          account.cloud_address,
          account.cloud_api_address,
        ],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }

  /// The cloud set in Settings › Orchester Cloud's advanced field; `None` for the build's own.
  pub fn cloud_address(&self) -> Result<Option<StoredCloudAddress>, StoreError> {
    let (web, api) = self.connection.query_row(
      "SELECT cloud_address, cloud_api_address FROM app_settings WHERE id = 1",
      [],
      |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
    )?;
    Ok(web.map(|web| StoredCloudAddress { web, api }))
  }

  pub fn set_cloud_address(&mut self, address: Option<&StoredCloudAddress>) -> Result<(), StoreError> {
    let (web, api) = address.map_or((None, None), |address| {
      (Some(address.web.as_str()), address.api.as_deref())
    });
    self.connection.execute(
      "UPDATE app_settings SET cloud_address = ?1, cloud_api_address = ?2 WHERE id = 1",
      params![web, api],
    )?;
    Ok(())
  }
}
