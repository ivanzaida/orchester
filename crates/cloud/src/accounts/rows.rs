use std::time::SystemTime;

use deadpool_postgres::GenericClient;
use orchester_api::cloud::{CloudAccount, CloudDevice, CloudPlatform};
use orchester_core::id::{CloudAccountId, CloudDeviceId};
use tokio_postgres::Row;
use uuid::Uuid;

use crate::{
  clock::{DAY_MS, HOUR_MS},
  db::{millis, time},
  error::CloudError,
  passwords::PasswordHash,
};

/// A desktop's token ends after 90 days without use, as team servers' device tokens do; a browser's (`web`) 12 hours
/// after it signed in (`docs/architecture/server.md`, "The cloud's web front").
const DEVICE_IDLE_MS: i64 = 90 * DAY_MS;
const WEB_SESSION_MS: i64 = 12 * HOUR_MS;

/// The condition of a device that is still signed in, given `$2` and `$3` from `active_since`.
pub(super) const ACTIVE: &str =
  "revoked_at IS NULL AND CASE WHEN platform = 'web' THEN created_at > $3 ELSE last_used_at > $2 END";

/// The two cutoffs of `ACTIVE` at `now`: last use for desktops, sign-in for browsers.
pub(super) fn active_since(now: i64) -> (SystemTime, SystemTime) {
  (time(now - DEVICE_IDLE_MS), time(now - WEB_SESSION_MS))
}

/// The device a request's token belongs to, unrevoked and in use when the request came.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Authenticated {
  pub account: CloudAccountId,
  pub device: CloudDeviceId,
}

pub(super) struct AccountRow {
  pub account: CloudAccount,
  pub hash: PasswordHash,
  pub confirmed: bool,
}

const ACCOUNT_COLUMNS: &str = "id, email, display_name, created_at, password_hash, confirmed_at";

pub(super) async fn account_by_email(
  client: &impl GenericClient,
  email: &str,
) -> Result<Option<AccountRow>, CloudError> {
  let row = client
    .query_opt(
      &format!("SELECT {ACCOUNT_COLUMNS} FROM accounts WHERE email = $1"),
      &[&email],
    )
    .await?;
  row.as_ref().map(account_row).transpose()
}

pub(super) async fn account(client: &impl GenericClient, id: CloudAccountId) -> Result<AccountRow, CloudError> {
  let row = client
    .query_one(
      &format!("SELECT {ACCOUNT_COLUMNS} FROM accounts WHERE id = $1"),
      &[&id.as_uuid()],
    )
    .await?;
  account_row(&row)
}

fn account_row(row: &Row) -> Result<AccountRow, CloudError> {
  Ok(AccountRow {
    account: CloudAccount {
      id: CloudAccountId::from_uuid(row.try_get(0)?),
      email: row.try_get(1)?,
      display_name: row.try_get(2)?,
      created_at: millis(row.try_get(3)?)?,
    },
    hash: PasswordHash::stored(row.try_get(4)?),
    confirmed: row.try_get::<_, Option<SystemTime>>(5)?.is_some(),
  })
}

pub(super) const DEVICE_COLUMNS: &str = "id, name, platform, created_at, last_used_at";

pub(super) fn device_row(row: &Row, current: CloudDeviceId) -> Result<CloudDevice, CloudError> {
  let id = CloudDeviceId::from_uuid(row.try_get::<_, Uuid>(0)?);
  Ok(CloudDevice {
    id,
    name: row.try_get(1)?,
    platform: CloudPlatform::parse(row.try_get(2)?).ok_or(CloudError::StoredData)?,
    created_at: millis(row.try_get(3)?)?,
    last_used_at: millis(row.try_get(4)?)?,
    current: id == current,
  })
}

/// Revokes every device of the account but `keep`.
pub(super) async fn revoke_devices(
  client: &impl GenericClient,
  account: CloudAccountId,
  keep: Option<CloudDeviceId>,
  now: i64,
) -> Result<u64, CloudError> {
  Ok(
    client
      .execute(
        "UPDATE devices SET revoked_at = $1 WHERE account = $2 AND revoked_at IS NULL AND id IS DISTINCT FROM $3",
        &[&time(now), &account.as_uuid(), &keep.map(CloudDeviceId::as_uuid)],
      )
      .await?,
  )
}
