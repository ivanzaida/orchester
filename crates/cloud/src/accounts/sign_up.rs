use std::net::IpAddr;

use deadpool_postgres::GenericClient;
use orchester_api::{
  Credential,
  cloud::{CloudErrorCode, SignUp},
};
use orchester_core::id::CloudAccountId;
use uuid::Uuid;

use super::{
  Cloud, input,
  links::{LinkPurpose, new_link, use_link},
};
use crate::{
  db::{audit, time},
  error::CloudError,
  mail::Email,
  passwords::Passwords,
};

impl Cloud {
  /// Answers alike whether or not the email has an account: a new one gets a confirmation link, an unconfirmed one a
  /// new link (its password stays: a stranger's sign-up must not set it), a confirmed one a note that it exists.
  pub(crate) async fn sign_up(&self, input: SignUp, address: IpAddr) -> Result<(), CloudError> {
    let email = input::email(&input.email)?;
    let display_name = input::display_name(&input.display_name)?;
    Passwords::check(&input.password)?;
    self.inner.limits.email(&email, address, self.now())?;
    // Hashed for every sign-up, so a registered email takes as long as a new one.
    let hash = self.inner.passwords.hash(input.password).await?;
    let now = self.now();
    let mut client = self.inner.db.client().await?;
    let transaction = client.transaction().await?;
    let id = CloudAccountId::generate();
    let created = transaction
      .execute(
        "INSERT INTO accounts (id, email, display_name, password_hash, created_at, password_changed_at)
         VALUES ($1, $2, $3, $4, $5, $5) ON CONFLICT (email) DO NOTHING",
        &[&id.as_uuid(), &email, &display_name, &hash.expose(), &time(now)],
      )
      .await?
      == 1;
    let message = if created {
      audit(&transaction, Some(id), "signed_up", &id.to_string(), Some(address), now).await?;
      let token = new_link(&transaction, id, LinkPurpose::Confirm, now).await?;
      self.inner.messages.confirm(email, &display_name, &token)
    } else {
      self.repeated_sign_up(&transaction, email, address, now).await?
    };
    transaction.commit().await?;
    self.send_later(message);
    Ok(())
  }

  async fn repeated_sign_up(
    &self,
    client: &impl GenericClient,
    email: String,
    address: IpAddr,
    now: i64,
  ) -> Result<Email, CloudError> {
    let row = client
      .query_one(
        "SELECT id, display_name, confirmed_at IS NOT NULL FROM accounts WHERE email = $1",
        &[&email],
      )
      .await?;
    let id = CloudAccountId::from_uuid(row.try_get::<_, Uuid>(0)?);
    audit(
      client,
      Some(id),
      "sign_up_repeated",
      &id.to_string(),
      Some(address),
      now,
    )
    .await?;
    if row.try_get(2)? {
      return Ok(self.inner.messages.already_registered(email));
    }
    let token = new_link(client, id, LinkPurpose::Confirm, now).await?;
    let display_name: String = row.try_get(1)?;
    Ok(self.inner.messages.confirm(email, &display_name, &token))
  }

  pub(crate) async fn confirm(&self, token: Credential, address: IpAddr) -> Result<(), CloudError> {
    self.inner.limits.link_finish(address, self.now())?;
    let now = self.now();
    let mut client = self.inner.db.client().await?;
    let transaction = client.transaction().await?;
    let Some(account) = use_link(&transaction, &token, LinkPurpose::Confirm, now).await? else {
      self.inner.limits.link_failed(address, now);
      return Err(CloudErrorCode::InvalidLink.into());
    };
    transaction
      .execute(
        "UPDATE accounts SET confirmed_at = coalesce(confirmed_at, $1) WHERE id = $2",
        &[&time(now), &account.as_uuid()],
      )
      .await?;
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
