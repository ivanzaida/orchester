use std::net::IpAddr;

use orchester_api::{PasswordChange, cloud::CloudErrorCode};

use super::{
  Authenticated, Cloud,
  links::end_links,
  rows::{account, revoke_devices},
};
use crate::{
  db::{audit, time},
  error::CloudError,
};

impl Cloud {
  /// Changes the password with the current one and signs out the account's other devices. Failed current passwords
  /// count against the account as sign-in failures count against an email.
  pub(crate) async fn change_password(
    &self,
    auth: Authenticated,
    input: PasswordChange,
    address: IpAddr,
  ) -> Result<(), CloudError> {
    let attempt = self.inner.limits.sign_in.password(auth.account, self.now())?;
    let previous = account(&self.inner.db.client().await?, auth.account).await?.hash;
    let (verified, attempt) = self
      .inner
      .passwords
      .verify(
        input.current_password,
        previous.clone(),
        attempt,
        self.inner.clock.as_ref(),
      )
      .await?;
    if !verified {
      let now = self.now();
      attempt.failed(now);
      let client = self.inner.db.client().await?;
      audit(
        &client,
        Some(auth.account),
        "password_change_failed",
        &auth.device.to_string(),
        Some(address),
        now,
      )
      .await?;
      return Err(CloudErrorCode::InvalidCredentials.into());
    }
    let new_hash = match self.inner.passwords.hash(input.new_password).await {
      Ok(hash) => hash,
      Err(error) => {
        attempt.succeeded();
        return Err(error);
      }
    };
    let now = self.now();
    let mut client = self.inner.db.client().await?;
    let transaction = client.transaction().await?;
    // Compare and set: a reset or another change meanwhile makes this one fail rather than overwrite it.
    let changed = transaction
      .execute(
        "UPDATE accounts SET password_hash = $1, password_changed_at = $2 WHERE id = $3 AND password_hash = $4",
        &[
          &new_hash.expose(),
          &time(now),
          &auth.account.as_uuid(),
          &previous.expose(),
        ],
      )
      .await?;
    if changed == 0 {
      attempt.failed(now);
      return Err(CloudErrorCode::InvalidCredentials.into());
    }
    attempt.succeeded();
    end_links(&transaction, auth.account, None, now).await?;
    revoke_devices(&transaction, auth.account, Some(auth.device), now).await?;
    audit(
      &transaction,
      Some(auth.account),
      "password_changed",
      &auth.device.to_string(),
      Some(address),
      now,
    )
    .await?;
    transaction.commit().await?;
    Ok(())
  }
}
