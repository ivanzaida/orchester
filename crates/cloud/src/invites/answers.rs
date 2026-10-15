//! The invitee's side: the open invites to the account's email, declining one, and accepting one, which hands the app
//! the server to connect to and an identity token for it that names the invite.

use orchester_api::cloud::{CloudErrorCode, CloudInvite, InviteAccepted, InvitedProject};
use orchester_core::id::InviteId;
use tokio_postgres::Row;

use super::{parse_kind, parse_role};
use crate::{
  Cloud,
  accounts::Authenticated,
  db::{audit, millis, time},
  error::CloudError,
  servers::{joined, server_row},
};

/// The columns `invite_row` reads, from `invites i` and `servers s`.
const COLUMNS: &str = "i.id, i.kind, i.role, i.admin, i.project_name, i.project_address, i.inviter_display_name,
  i.expires_at, s.id, s.name, s.address, s.fingerprint";

impl Cloud {
  /// The open invites to the account's email, oldest first.
  pub(crate) async fn invites(&self, auth: Authenticated) -> Result<Vec<CloudInvite>, CloudError> {
    let client = self.inner.db.client().await?;
    let rows = client
      .query(
        &format!(
          "SELECT {COLUMNS} FROM invites i JOIN servers s ON s.id = i.server JOIN accounts a ON a.email = i.email
           WHERE a.id = $1 AND i.expires_at > $2 ORDER BY i.received_at, i.id"
        ),
        &[&auth.account.as_uuid(), &time(self.now())],
      )
      .await?;
    rows.iter().map(invite_row).collect()
  }

  /// Deletes an invite to the account's email. The team server keeps its own copy until it expires; nothing joins
  /// without an identity token that names the invite, and only accepting gives one.
  pub(crate) async fn decline_invite(&self, auth: Authenticated, id: InviteId) -> Result<(), CloudError> {
    let now = self.now();
    let client = self.inner.db.client().await?;
    let deleted = client
      .execute(
        "DELETE FROM invites i USING accounts a WHERE i.id = $1 AND a.id = $2 AND i.email = a.email",
        &[&id.as_uuid(), &auth.account.as_uuid()],
      )
      .await?;
    if deleted == 0 {
      return Err(CloudErrorCode::NotFound.into());
    }
    audit(
      &client,
      Some(auth.account),
      "invite_declined",
      &id.to_string(),
      None,
      now,
    )
    .await
  }

  /// Ends an open invite to the account's email and gives what joining needs: the server, the project, and a
  /// five-minute identity token for that server naming the invite. The account counts as having joined the server.
  /// Without a published key set nothing changes and the answer is `keys_unavailable`.
  pub(crate) async fn accept_invite(&self, auth: Authenticated, id: InviteId) -> Result<InviteAccepted, CloudError> {
    self.inner.limits.identity_token(auth.device, self.now())?;
    let now = self.now();
    let mut client = self.inner.db.client().await?;
    let transaction = client.transaction().await?;
    let row = transaction
      .query_opt(
        &format!(
          "DELETE FROM invites i USING accounts a, servers s
           WHERE i.id = $1 AND a.id = $2 AND i.email = a.email AND s.id = i.server AND i.expires_at > $3
           RETURNING {COLUMNS}"
        ),
        &[&id.as_uuid(), &auth.account.as_uuid(), &time(now)],
      )
      .await?
      .ok_or(CloudErrorCode::NotFound)?;
    let invite = invite_row(&row)?;
    joined(&transaction, invite.server.id, auth, now).await?;
    let issued = self
      .identity_token_for(&transaction, auth, invite.server.id.to_string(), Some(id.to_string()))
      .await?;
    audit(
      &transaction,
      Some(auth.account),
      "invite_accepted",
      &id.to_string(),
      None,
      now,
    )
    .await?;
    transaction.commit().await?;
    Ok(InviteAccepted {
      server: invite.server,
      project: invite.project,
      identity_token: issued.identity_token,
      expires_at: issued.expires_at,
    })
  }
}

fn invite_row(row: &Row) -> Result<CloudInvite, CloudError> {
  let project = match (
    row.try_get::<_, Option<String>>(4)?,
    row.try_get::<_, Option<String>>(5)?,
  ) {
    (Some(name), Some(address)) => Some(InvitedProject { address, name }),
    (None, None) => None,
    _ => return Err(CloudError::StoredData),
  };
  let role = row
    .try_get::<_, Option<String>>(2)?
    .map(|role| parse_role(&role).ok_or(CloudError::StoredData))
    .transpose()?;
  Ok(CloudInvite {
    id: InviteId::from_uuid(row.try_get(0)?),
    kind: parse_kind(row.try_get(1)?).ok_or(CloudError::StoredData)?,
    role,
    admin: row.try_get(3)?,
    server: server_row(row, 8)?,
    project,
    inviter_display_name: row.try_get(6)?,
    expires_at: millis(row.try_get(7)?)?,
  })
}
