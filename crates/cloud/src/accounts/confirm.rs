use std::net::IpAddr;

use orchester_api::cloud::{CloudErrorCode, ConfirmEmail};

use super::{
  Cloud,
  links::{LinkPurpose, end_links, lock_account, pending_link, use_link},
};
use crate::{
  db::{audit, time},
  error::CloudError,
  passwords::PasswordHash,
  tokens::token_hash,
};

impl Cloud {
  /// Confirms the email with the password of the sign-up the link was sent for (review 2, N1) and gives the account
  /// that sign-up's password and display name; the account's other confirmation links end. An unknown, used or
  /// expired link and a wrong password answer alike, after the same password check, and count against the client's
  /// address and against the link.
  pub(crate) async fn confirm(&self, input: ConfirmEmail, address: IpAddr) -> Result<(), CloudError> {
    let link_key = token_hash(&input.link_token);
    self.inner.limits.link_finish(address, &link_key, self.now())?;
    let mut client = self.inner.db.client().await?;
    let pending = pending_link(&client, &input.link_token, LinkPurpose::Confirm, self.now()).await?;
    let (account, hash) = pending.map_or((None, None), |(account, hash)| (Some(account), hash));
    let verified = self.inner.passwords.matches(input.password, hash).await?;
    let now = self.now();
    let refused = || {
      self.inner.limits.link_failed(address, &link_key, now);
      CloudError::from(CloudErrorCode::InvalidLink)
    };
    let Some(account) = account.filter(|_| verified) else {
      return Err(refused());
    };
    // The account row first, then its links: the order sign-up and reset take too (review 2, N3).
    let transaction = client.transaction().await?;
    if !lock_account(&transaction, account).await? {
      return Err(refused());
    }
    let Some(link) = use_link(&transaction, &input.link_token, LinkPurpose::Confirm, now).await? else {
      return Err(refused());
    };
    transaction
      .execute(
        "UPDATE accounts SET confirmed_at = $1, password_changed_at = $1,
           password_hash = coalesce($3, password_hash), display_name = coalesce($4, display_name)
         WHERE id = $2 AND confirmed_at IS NULL",
        &[
          &time(now),
          &account.as_uuid(),
          &link.hash.as_ref().map(PasswordHash::expose),
          &link.display_name,
        ],
      )
      .await?;
    end_links(&transaction, account, Some(LinkPurpose::Confirm), now).await?;
    audit(
      &transaction,
      Some(account),
      "email_confirmed",
      &account.to_string(),
      Some(address),
      now,
    )
    .await?;
    transaction.commit().await?;
    Ok(())
  }
}
