use std::net::IpAddr;

use deadpool_postgres::GenericClient;
use orchester_api::cloud::SignUp;
use orchester_core::id::CloudAccountId;
use uuid::Uuid;

use super::{
  Cloud, input,
  links::{NewLink, Pending, new_link},
};
use crate::{
  db::{audit, time},
  error::CloudError,
  mail::Email,
  passwords::Passwords,
};

impl Cloud {
  /// Answers alike whether or not the email has an account. A new email gets an unconfirmed account and a
  /// confirmation link; an unconfirmed one gets one more link; a confirmed one gets a note that it exists. Each link
  /// carries its sign-up's password and display name, and confirming it asks for that password (review 2, N1), so
  /// only the person who chose a password and reads the email can give the account that password.
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
    // Earlier links stay: each carries its own sign-up's password, which confirming it asks for (review 2, N1).
    let token = new_link(client, id, NewLink::Confirm(pending), now).await?;
    Ok(self.inner.messages.confirm(email, &token))
  }
}
