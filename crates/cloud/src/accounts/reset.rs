use std::net::IpAddr;

use deadpool_postgres::GenericClient;
use orchester_api::cloud::{CloudErrorCode, ResetFinish, ResetRequest};
use orchester_core::id::CloudAccountId;

use super::{
  Cloud, input,
  links::{LinkPurpose, link_usable, new_link, use_link},
  rows::{account_by_email, revoke_devices},
};
use crate::{
  db::{audit, time},
  error::CloudError,
  passwords::Passwords,
};

impl Cloud {
  /// Sends a reset link when the email has an account; the answer is the same for any email.
  pub(crate) async fn request_reset(&self, input: ResetRequest, address: IpAddr) -> Result<(), CloudError> {
    let email = input::email(&input.email)?;
    self.inner.limits.email(&email, address, self.now())?;
    let now = self.now();
    let mut client = self.inner.db.client().await?;
    let transaction = client.transaction().await?;
    let Some(account) = account_by_email(&transaction, &email).await? else {
      return Ok(());
    };
    let id = account.account.id;
    let token = new_link(&transaction, id, LinkPurpose::Reset, now).await?;
    audit(
      &transaction,
      Some(id),
      "reset_requested",
      &id.to_string(),
      Some(address),
      now,
    )
    .await?;
    transaction.commit().await?;
    self.send_later(self.inner.messages.reset(email, &token));
    Ok(())
  }

  /// Sets the new password, confirms the email (the link proves it), ends the account's other reset links and signs
  /// out every device.
  pub(crate) async fn finish_reset(&self, input: ResetFinish, address: IpAddr) -> Result<(), CloudError> {
    self.inner.limits.link_finish(address, self.now())?;
    Passwords::check(&input.new_password)?;
    let usable = link_usable(
      &self.inner.db.client().await?,
      &input.link_token,
      LinkPurpose::Reset,
      self.now(),
    )
    .await?;
    if !usable {
      self.inner.limits.link_failed(address, self.now());
      return Err(CloudErrorCode::InvalidLink.into());
    }
    let hash = self.inner.passwords.hash(input.new_password).await?;
    let now = self.now();
    let mut client = self.inner.db.client().await?;
    let transaction = client.transaction().await?;
    let Some(account) = use_link(&transaction, &input.link_token, LinkPurpose::Reset, now).await? else {
      self.inner.limits.link_failed(address, now);
      return Err(CloudErrorCode::InvalidLink.into());
    };
    transaction
      .execute(
        "UPDATE accounts SET password_hash = $1, password_changed_at = $2, confirmed_at = coalesce(confirmed_at, $2)
         WHERE id = $3",
        &[&hash.expose(), &time(now), &account.as_uuid()],
      )
      .await?;
    end_reset_links(&transaction, account, now).await?;
    revoke_devices(&transaction, account, None, now).await?;
    audit(
      &transaction,
      Some(account),
      "password_reset",
      &account.to_string(),
      Some(address),
      now,
    )
    .await?;
    transaction.commit().await?;
    Ok(())
  }
}

/// A password set by a reset or a change leaves no older reset link usable.
pub(super) async fn end_reset_links(
  client: &impl GenericClient,
  account: CloudAccountId,
  now: i64,
) -> Result<(), CloudError> {
  client
    .execute(
      "UPDATE email_links SET used_at = $1 WHERE account = $2 AND purpose = 'reset' AND used_at IS NULL",
      &[&time(now), &account.as_uuid()],
    )
    .await?;
  Ok(())
}
