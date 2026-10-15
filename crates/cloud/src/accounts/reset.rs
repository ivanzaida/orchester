use std::net::IpAddr;

use orchester_api::cloud::{CloudErrorCode, ResetFinish, ResetRequest};

use super::{
  Cloud, input,
  links::{LinkPurpose, NewLink, end_links, link_usable, new_link, use_link},
  rows::{account_by_email, revoke_devices},
};
use crate::{
  db::{audit, time},
  error::CloudError,
  passwords::Passwords,
};

impl Cloud {
  /// Answers before touching the database, the same for any email; the lookup, the link and the email happen in the
  /// background, so the answer's time says nothing about whether the email has an account (review 1, M5).
  pub(crate) async fn request_reset(&self, input: ResetRequest, address: IpAddr) -> Result<(), CloudError> {
    let email = input::email(&input.email)?;
    self.inner.limits.email(&email, address, self.now())?;
    let cloud = self.clone();
    tokio::spawn(async move {
      if let Err(error) = cloud.send_reset(email, address).await {
        error.log();
      }
    });
    Ok(())
  }

  async fn send_reset(&self, email: String, address: IpAddr) -> Result<(), CloudError> {
    let now = self.now();
    let mut client = self.inner.db.client().await?;
    let transaction = client.transaction().await?;
    let Some(account) = account_by_email(&transaction, &email).await? else {
      return Ok(());
    };
    let id = account.account.id;
    let token = new_link(&transaction, id, NewLink::Reset, now).await?;
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
    let Some(link) = use_link(&transaction, &input.link_token, LinkPurpose::Reset, now).await? else {
      self.inner.limits.link_failed(address, now);
      return Err(CloudErrorCode::InvalidLink.into());
    };
    let account = link.account;
    transaction
      .execute(
        "UPDATE accounts SET password_hash = $1, password_changed_at = $2, confirmed_at = coalesce(confirmed_at, $2)
         WHERE id = $3",
        &[&hash.expose(), &time(now), &account.as_uuid()],
      )
      .await?;
    end_links(&transaction, account, None, now).await?;
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
