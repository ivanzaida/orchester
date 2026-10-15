//! Invites in flight (`docs/architecture/server.md`, "Invites"): a team server signs an invite by email with its own
//! key, and it or the inviter's browser delivers it here. The cloud checks the signature against the key the server
//! linked with, keeps the invite until it expires or the server revokes it, emails the invitee in its own words, and
//! lists it to the account with that email.

use deadpool_postgres::GenericClient;
use orchester_api::{
  InviteKind, MemberRole,
  cloud::{CloudErrorCode, CloudInvite, InviteDelivered, InviteWithdrawn, InvitingServer},
  identity::{CLOCK_SKEW_SECS, PublicKey, read_statement, verify_statement},
  statements::{INVITE_REVOCATION_TYPE, InviteRevocation, InvitedProject, SIGNED_INVITE_TYPE, SignedInvite},
};
use orchester_core::id::{InviteId, TeamServerId};
use serde::de::DeserializeOwned;
use tokio_postgres::Row;

use crate::{
  Cloud,
  accounts::Authenticated,
  clock::DAY_MS,
  db::{audit, millis, time},
  error::CloudError,
  servers,
};

/// Invites one server may send through the cloud in a day, whatever its own limits say.
const PER_SERVER_PER_DAY: i64 = 200;
/// An invite by email is open at most this long (the team server's 7 days, and a day of clock difference).
const MAX_LIFETIME_MS: i64 = 8 * DAY_MS;

impl Cloud {
  /// `POST /invites`: an invite a linked server signed. Delivering one twice (from the server and from a browser)
  /// keeps one and sends one email.
  pub(crate) async fn receive_invite(&self, input: InviteDelivered) -> Result<(), CloudError> {
    let now = self.now();
    let client = self.client().await?;
    let invite: SignedInvite = verified(&client, &input.invite, SIGNED_INVITE_TYPE, |invite: &SignedInvite| {
      invite.server
    })
    .await?;
    let email = invite.email.trim().to_lowercase();
    let fresh = invite.issued_at <= now + CLOCK_SKEW_SECS * 1000 && invite.expires_at > now;
    if !fresh || invite.expires_at > now + MAX_LIFETIME_MS || !email.contains('@') || email.len() > 254 {
      return Err(CloudErrorCode::Invalid.into());
    }
    let sent_today: i64 = client
      .query_one(
        "SELECT count(*) FROM invites WHERE server = $1 AND received_at > $2",
        &[&invite.server.as_uuid(), &time(now - DAY_MS)],
      )
      .await?
      .try_get(0)?;
    if sent_today >= PER_SERVER_PER_DAY {
      return Err(CloudError::RateLimited(3600));
    }
    let known = client
      .query_opt("SELECT 1 FROM invites WHERE id = $1", &[&invite.id.as_uuid()])
      .await?
      .is_some();
    if known {
      return Ok(());
    }
    self.inner.limits.invite_email(&email, now)?;
    if !store(&client, &invite, &email, now).await? {
      return Ok(());
    }
    audit(&client, None, "invite_received", &invite.id.to_string(), None, now).await?;
    let registered = client
      .query_opt(
        "SELECT 1 FROM accounts WHERE email = $1 AND confirmed_at IS NOT NULL",
        &[&email],
      )
      .await?
      .is_some();
    let message = if registered {
      self.inner.messages.invite(email)
    } else {
      self.inner.messages.invite_sign_up(email, invite.id)
    };
    self.send_later(message);
    Ok(())
  }

  /// `DELETE /invites/{id}`: the server that made the invite revoked it.
  pub(crate) async fn withdraw_invite(&self, id: InviteId, input: InviteWithdrawn) -> Result<(), CloudError> {
    let client = self.client().await?;
    let revocation: InviteRevocation = verified(
      &client,
      &input.revocation,
      INVITE_REVOCATION_TYPE,
      |revocation: &InviteRevocation| revocation.server,
    )
    .await?;
    if revocation.id != id {
      return Err(CloudErrorCode::Invalid.into());
    }
    client
      .execute(
        "DELETE FROM invites WHERE id = $1 AND server = $2",
        &[&id.as_uuid(), &revocation.server.as_uuid()],
      )
      .await?;
    audit(&client, None, "invite_withdrawn", &id.to_string(), None, self.now()).await
  }

  /// `GET /invites`: the open invites to the account's email from linked servers, soonest to expire first.
  pub(crate) async fn invites(&self, auth: Authenticated) -> Result<Vec<CloudInvite>, CloudError> {
    let rows = self
      .client()
      .await?
      .query(
        "SELECT i.id, i.kind, i.role, s.id, s.name, s.address, i.project_address, i.project_name,
           i.inviter_display_name, i.expires_at
         FROM invites i JOIN servers s ON s.id = i.server JOIN accounts a ON a.email = i.email
         WHERE a.id = $1 AND s.unlinked_at IS NULL AND i.expires_at > $2
         ORDER BY i.expires_at, i.id",
        &[&auth.account.as_uuid(), &time(self.now())],
      )
      .await?;
    rows.iter().map(invite_row).collect()
  }
}

/// A statement of type `typ` that the linked server it names (`server`) signed with the key it linked with.
async fn verified<T: DeserializeOwned>(
  client: &impl GenericClient,
  compact: &str,
  typ: &str,
  server: impl Fn(&T) -> TeamServerId,
) -> Result<T, CloudError> {
  let invalid = || CloudError::from(CloudErrorCode::Invalid);
  let unverified: T = read_statement(compact, typ).map_err(|_| invalid())?;
  let row = servers::linked(client, server(&unverified).as_uuid())
    .await?
    .ok_or_else(invalid)?;
  let key = PublicKey::parse(row.try_get::<_, &str>(4)?).map_err(|_| CloudError::StoredData)?;
  verify_statement(compact, typ, &key).map_err(|_| invalid())
}

/// Keeps the invite; `false` when it was here already.
async fn store(client: &impl GenericClient, invite: &SignedInvite, email: &str, now: i64) -> Result<bool, CloudError> {
  let project = invite.project.as_ref();
  let stored = client
    .execute(
      "INSERT INTO invites (id, server, kind, email, project_address, project_name, role, inviter_display_name,
         expires_at, received_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (id) DO NOTHING",
      &[
        &invite.id.as_uuid(),
        &invite.server.as_uuid(),
        &kind_name(invite.kind),
        &email,
        &project.map(|project| project.address.as_str()),
        &project.map(|project| project.name.as_str()),
        &invite.role.map(role_name),
        &invite.inviter.trim(),
        &time(invite.expires_at),
        &time(now),
      ],
    )
    .await?;
  Ok(stored == 1)
}

fn invite_row(row: &Row) -> Result<CloudInvite, CloudError> {
  let project = match (
    row.try_get::<_, Option<String>>(6)?,
    row.try_get::<_, Option<String>>(7)?,
  ) {
    (Some(address), Some(name)) => Some(InvitedProject { address, name }),
    _ => None,
  };
  Ok(CloudInvite {
    id: InviteId::from_uuid(row.try_get(0)?),
    kind: parse_kind(row.try_get(1)?)?,
    role: row.try_get::<_, Option<&str>>(2)?.map(parse_role).transpose()?,
    server: InvitingServer {
      id: TeamServerId::from_uuid(row.try_get(3)?),
      name: row.try_get(4)?,
      address: row.try_get(5)?,
    },
    project,
    inviter_display_name: row.try_get(8)?,
    expires_at: millis(row.try_get(9)?)?,
  })
}

const fn kind_name(kind: InviteKind) -> &'static str {
  match kind {
    InviteKind::Server => "server",
    InviteKind::Project => "project",
    InviteKind::Account => "account",
  }
}

fn parse_kind(value: &str) -> Result<InviteKind, CloudError> {
  [InviteKind::Server, InviteKind::Project, InviteKind::Account]
    .into_iter()
    .find(|kind| kind_name(*kind) == value)
    .ok_or(CloudError::StoredData)
}

const fn role_name(role: MemberRole) -> &'static str {
  match role {
    MemberRole::Owner => "owner",
    MemberRole::Member => "member",
  }
}

fn parse_role(value: &str) -> Result<MemberRole, CloudError> {
  [MemberRole::Owner, MemberRole::Member]
    .into_iter()
    .find(|role| role_name(*role) == value)
    .ok_or(CloudError::StoredData)
}
