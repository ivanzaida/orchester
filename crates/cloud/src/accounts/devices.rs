use orchester_api::cloud::{AccountChanges, AccountResponse, CloudDevice, CloudErrorCode};
use orchester_core::id::CloudDeviceId;

use super::{
  Authenticated, Cloud, input,
  rows::{ACTIVE, DEVICE_COLUMNS, account, active_since, device_row},
  two_factor,
};
use crate::{
  db::{audit, time},
  error::CloudError,
};

impl Cloud {
  /// The account, with its two-factor authentication as the asking device sees it.
  pub(crate) async fn me(&self, auth: Authenticated) -> Result<AccountResponse, CloudError> {
    let client = self.inner.db.client().await?;
    Ok(AccountResponse {
      account: account(&client, auth.account).await?.account,
      two_factor: two_factor::status(&client, auth).await?,
    })
  }

  /// Only the display name changes here; a new email (confirmed first) is not built yet and is refused as an
  /// unknown field.
  pub(crate) async fn change_account(
    &self,
    auth: Authenticated,
    changes: AccountChanges,
  ) -> Result<AccountResponse, CloudError> {
    let client = self.inner.db.client().await?;
    if let Some(name) = changes.display_name {
      let name = input::display_name(&name)?;
      client
        .execute(
          "UPDATE accounts SET display_name = $1 WHERE id = $2",
          &[&name, &auth.account.as_uuid()],
        )
        .await?;
    }
    drop(client);
    self.me(auth).await
  }

  /// The account's signed-in devices, oldest first; `current` marks the one asking.
  pub(crate) async fn devices(&self, auth: Authenticated) -> Result<Vec<CloudDevice>, CloudError> {
    let (idle, web) = active_since(self.now());
    let client = self.inner.db.client().await?;
    let rows = client
      .query(
        &format!(
          "SELECT {DEVICE_COLUMNS} FROM devices WHERE account = $1 AND {ACTIVE}
           ORDER BY created_at, id"
        ),
        &[&auth.account.as_uuid(), &idle, &web],
      )
      .await?;
    rows.iter().map(|row| device_row(row, auth.device)).collect()
  }

  /// Signs a device of the account out; `None` is the device asking.
  pub(crate) async fn revoke_device(
    &self,
    auth: Authenticated,
    device: Option<CloudDeviceId>,
  ) -> Result<(), CloudError> {
    let device = device.unwrap_or(auth.device);
    let now = self.now();
    let client = self.inner.db.client().await?;
    let revoked = client
      .execute(
        "UPDATE devices SET revoked_at = $1 WHERE id = $2 AND account = $3 AND revoked_at IS NULL",
        &[&time(now), &device.as_uuid(), &auth.account.as_uuid()],
      )
      .await?;
    if revoked == 0 {
      return Err(CloudErrorCode::NotFound.into());
    }
    audit(
      &client,
      Some(auth.account),
      "device_revoked",
      &device.to_string(),
      None,
      now,
    )
    .await
  }
}
