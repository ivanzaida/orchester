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
  links::{LinkPurpose, NewLink, Pending, end_links, new_link, use_link},
};
use crate::{
  db::{audit, time},
  error::CloudError,
  mail::Email,
  passwords::{PasswordHash, Passwords},
};

impl Cloud {
  /// Answers alike whether or not the email has an account. A new email gets an unconfirmed account and a
  /// confirmation link; an unconfirmed one gets a new link that replaces the earlier ones; a confirmed one gets a
  /// note that it exists. The password and display name travel with the link, so the account takes those of the
  /// sign-up whose link is confirmed (review 1, H1).
  pub(crate) async fn sign_up(&self, input: SignUp, address: IpAddr) -> Result<(), CloudError> {
    let email = input::email(&input.email)?;
    let display_name = input::display_name(&input.display_name)?;
    Passwords::check(&input.password)?;
    self.inner.limits.email(&email, address, self.now())?;
    // Hashed for every sign-up, so a registered email takes as long as a new one.
    let hash = self.inner.passwords.hash(input.password).await?;
    let pending = Pending {
      hash: &hash,
      display_name: &display_name,
    };
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
      let token = new_link(&transaction, id, NewLink::Confirm(pending), now).await?;
      self.inner.messages.confirm(email, &token)
    } else {
      self
        .repeated_sign_up(&transaction, email, pending, address, now)
        .await?
    };
    transaction.commit().await?;
    self.send_later(message);
    Ok(())
  }

  async fn repeated_sign_up(
    &self,
    client: &impl GenericClient,
    email: String,
    pending: Pending<'_>,
    address: IpAddr,
    now: i64,
  ) -> Result<Email, CloudError> {
    // The row lock orders this with a confirmation of the same account.
    let row = client
      .query_one(
        "SELECT id, confirmed_at IS NOT NULL FROM accounts WHERE email = $1 FOR UPDATE",
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
    if row.try_get(1)? {
      return Ok(self.inner.messages.already_registered(email));
    }
    end_links(client, id, Some(LinkPurpose::Confirm), now).await?;
    let token = new_link(client, id, NewLink::Confirm(pending), now).await?;
    Ok(self.inner.messages.confirm(email, &token))
  }

  /// Confirms the email and gives the account the password and display name of the sign-up the link was sent for.
  /// A link of an account that is confirmed already (by a reset meanwhile) changes nothing.
  pub(crate) async fn confirm(&self, token: Credential, address: IpAddr) -> Result<(), CloudError> {
    self.inner.limits.link_finish(address, self.now())?;
    let now = self.now();
    let mut client = self.inner.db.client().await?;
    let transaction = client.transaction().await?;
    let Some(link) = use_link(&transaction, &token, LinkPurpose::Confirm, now).await? else {
      self.inner.limits.link_failed(address, now);
      return Err(CloudErrorCode::InvalidLink.into());
    };
    let account = link.account;
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
