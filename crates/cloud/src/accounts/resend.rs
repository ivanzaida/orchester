use std::net::IpAddr;

use orchester_api::cloud::ResendConfirmation;
use orchester_core::id::CloudAccountId;
use uuid::Uuid;

use super::{Cloud, input, links::renew_confirm_link};
use crate::{db::audit, error::CloudError};

impl Cloud {
  /// Answers before touching the database, the same for any email, as a reset request does (review 1, M5). In the
  /// background an unconfirmed account gets one more confirmation link, carrying its newest sign-up's password (decision
  /// 65); a confirmed one gets the *you already have an account* note; an unknown email gets nothing. Counts against
  /// the email limits like a sign-up.
  pub(crate) async fn resend_confirmation(&self, input: ResendConfirmation, address: IpAddr) -> Result<(), CloudError> {
    let email = input::email(&input.email)?;
    self.inner.limits.email(&email, address, self.now())?;
    let cloud = self.clone();
    tokio::spawn(async move {
      if let Err(error) = cloud.send_confirmation_again(email, address).await {
        error.log();
      }
    });
    Ok(())
  }

  async fn send_confirmation_again(&self, email: String, address: IpAddr) -> Result<(), CloudError> {
    let now = self.now();
    let mut client = self.inner.db.client().await?;
    let transaction = client.transaction().await?;
    // The account row first, as sign-up, confirmation and reset take it (review 2, N3).
    let row = transaction
      .query_opt(
        "SELECT id, confirmed_at IS NOT NULL FROM accounts WHERE email = $1 FOR UPDATE",
        &[&email],
      )
      .await?;
    let Some(row) = row else {
      return Ok(());
    };
    let id = CloudAccountId::from_uuid(row.try_get::<_, Uuid>(0)?);
    let message = if row.try_get(1)? {
      Some(self.inner.messages.already_registered(email))
    } else {
      renew_confirm_link(&transaction, id, now)
        .await?
        .map(|token| self.inner.messages.confirm(email, &token))
    };
    audit(
      &transaction,
      Some(id),
      "confirmation_resent",
      &id.to_string(),
      Some(address),
      now,
    )
    .await?;
    transaction.commit().await?;
    if let Some(message) = message {
      self.send_later(message);
    }
    Ok(())
  }
}
