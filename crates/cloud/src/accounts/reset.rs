use std::net::IpAddr;

use orchester_api::cloud::{CloudErrorCode, ResetFinish, ResetRequest};

use super::{
  Cloud, input,
  links::{LinkPurpose, NewLink, end_links, lock_account, new_link, pending_link, use_link},
  rows::{account_by_email, revoke_devices},
};
use crate::{
  db::{audit, time},
  error::CloudError,
  passwords::Passwords,
  tokens::token_hash,
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
    let link_key = token_hash(&input.link_token);
    self.inner.limits.link_finish(address, &link_key, self.now())?;
    Passwords::check(&input.new_password)?;
    let mut client = self.inner.db.client().await?;
    let refused = |now| {
      self.inner.limits.link_failed(address, &link_key, now);
      CloudError::from(CloudErrorCode::InvalidLink)
    };
    let pending = pending_link(&client, &input.link_token, LinkPurpose::Reset, self.now()).await?;
    let Some((account, _)) = pending else {
      return Err(refused(self.now()));
    };
    let hash = self.inner.passwords.hash(input.new_password).await?;
    let now = self.now();
    // The account row first, then its links, as confirm and sign-up take them (review 2, N3).
    let transaction = client.transaction().await?;
    if !lock_account(&transaction, account).await? {
      return Err(refused(now));
    }
    if use_link(&transaction, &input.link_token, LinkPurpose::Reset, now)
      .await?
      .is_none()
    {
      return Err(refused(now));
    }
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
