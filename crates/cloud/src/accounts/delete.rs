use std::net::IpAddr;

use orchester_api::cloud::{AccountDeletion, CloudErrorCode};

use super::{Authenticated, Cloud, links::lock_account, rows::account};
use crate::{db::audit, error::CloudError};

impl Cloud {
  /// Deletes the account with its password: its devices, links and joins go with it, and the open invites to its
  /// email; servers it linked stay listed for those who joined them, with no owner. Failed passwords count against the
  /// account as a password change's do. The audit keeps the account's id, as for every other action.
  pub(crate) async fn delete_account(
    &self,
    auth: Authenticated,
    input: AccountDeletion,
    address: IpAddr,
  ) -> Result<(), CloudError> {
    let attempt = self.inner.limits.sign_in.password(auth.account, self.now())?;
    let row = account(&self.inner.db.client().await?, auth.account).await?;
    let (verified, attempt) = self
      .inner
      .passwords
      .verify(input.password, row.hash, attempt, self.inner.clock.as_ref())
      .await?;
    let now = self.now();
    if !verified {
      attempt.failed(now);
      let client = self.inner.db.client().await?;
      audit(
        &client,
        Some(auth.account),
        "account_delete_failed",
        &auth.device.to_string(),
        Some(address),
        now,
      )
      .await?;
      return Err(CloudErrorCode::InvalidCredentials.into());
    }
    attempt.succeeded();
    let mut client = self.inner.db.client().await?;
    let transaction = client.transaction().await?;
    if !lock_account(&transaction, auth.account).await? {
      return Err(CloudErrorCode::Unauthorized.into());
    }
    transaction
      .execute("DELETE FROM invites WHERE email = $1", &[&row.account.email])
      .await?;
    transaction
      .execute("DELETE FROM accounts WHERE id = $1", &[&auth.account.as_uuid()])
      .await?;
    audit(
      &transaction,
      Some(auth.account),
      "account_deleted",
      &auth.account.to_string(),
      Some(address),
      now,
    )
    .await?;
    transaction.commit().await?;
    Ok(())
  }
}
