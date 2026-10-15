//! Invites in flight (`docs/architecture/server.md`, "Invites"): a team server signs each invite with its own key and
//! sends it here, or the inviter's browser carries it. The cloud accepts one the registered server's key signed,
//! sends one email either way (to the account page, or to sign up first), and keeps it only while it is open.

mod answers;

use orchester_api::{
  InviteKind, MemberRole,
  cloud::{CloudErrorCode, INVITE_STATEMENT_TYPE, InviteStatement, SignedInvite},
  identity::{PublicKey, unverified_statement, verify_statement},
};

use crate::{
  Cloud,
  accounts::input,
  clock::{DAY_MS, MINUTE_MS},
  db::{audit, time},
  error::CloudError,
  mail::InviteMail,
};

/// A team server keeps an invite by email open for 7 days; a statement that claims longer is refused.
const MAX_LIFETIME_MS: i64 = 7 * DAY_MS;
/// The clock difference between the cloud and a team server that an invite's times tolerate.
const SKEW_MS: i64 = 2 * MINUTE_MS;
const MAX_NAME_CHARS: usize = 256;
const MAX_ADDRESS_CHARS: usize = 2048;

impl Cloud {
  /// Takes an invite its registered server signed; the same invite again changes nothing and sends nothing. Answers
  /// alike whether or not the email has an account.
  pub(crate) async fn receive_invite(&self, input: SignedInvite) -> Result<(), CloudError> {
    let invalid = |_| CloudError::from(CloudErrorCode::Invalid);
    let named: InviteStatement = unverified_statement(&input.invite, INVITE_STATEMENT_TYPE).map_err(invalid)?;
    let client = self.inner.db.client().await?;
    let server = client
      .query_opt(
        "SELECT public_key, name FROM servers WHERE id = $1",
        &[&named.server.as_uuid()],
      )
      .await?
      .ok_or(CloudErrorCode::Invalid)?;
    let key = PublicKey::from_bytes(&server.try_get::<_, Vec<u8>>(0)?).map_err(|_| CloudError::StoredData)?;
    let invite: InviteStatement = verify_statement(&input.invite, INVITE_STATEMENT_TYPE, &key).map_err(invalid)?;
    let now = self.now();
    let checked = Checked::of(&invite, now)?;
    let known = client
      .query_opt("SELECT 1 FROM invites WHERE id = $1", &[&invite.id.as_uuid()])
      .await?
      .is_some();
    if known {
      return Ok(());
    }
    self
      .inner
      .limits
      .invite(invite.server, &checked.inviter, &checked.email, now)?;
    if !self.record(&client, &invite, &checked, now).await? {
      return Ok(());
    }
    let registered = client
      .query_opt(
        "SELECT 1 FROM accounts WHERE email = $1 AND confirmed_at IS NOT NULL",
        &[&checked.email],
      )
      .await?
      .is_some();
    audit(&client, None, "invite_received", &invite.id.to_string(), None, now).await?;
    let server_name: String = server.try_get(1)?;
    let mail = InviteMail {
      kind: invite.kind,
      inviter: &checked.inviter,
      server: &server_name,
      project: checked.project.as_ref().map(|(name, _)| name.as_str()),
    };
    self.send_later(self.inner.messages.invite(checked.email.clone(), &mail, registered));
    Ok(())
  }

  /// Stores the invite; false when another carrier stored it first.
  async fn record(
    &self,
    client: &impl deadpool_postgres::GenericClient,
    invite: &InviteStatement,
    checked: &Checked,
    now: i64,
  ) -> Result<bool, CloudError> {
    let (project_name, project_address) = checked.project.clone().unzip();
    let inserted = client
      .execute(
        "INSERT INTO invites (id, server, email, kind, role, admin, project_name, project_address,
           inviter_display_name, received_at, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) ON CONFLICT (id) DO NOTHING",
        &[
          &invite.id.as_uuid(),
          &invite.server.as_uuid(),
          &checked.email,
          &kind_name(invite.kind),
          &invite.role.map(role_name),
          &invite.admin,
          &project_name,
          &project_address,
          &checked.inviter,
          &time(now),
          &time(invite.expires_at),
        ],
      )
      .await?;
    Ok(inserted == 1)
  }
}

/// What the cloud keeps of a signed invite, checked: the email as accounts keep it, the inviter's name, and the
/// project's name and address for a project invite.
struct Checked {
  email: String,
  inviter: String,
  project: Option<(String, String)>,
}

impl Checked {
  fn of(invite: &InviteStatement, now: i64) -> Result<Self, CloudError> {
    let shape = match invite.kind {
      InviteKind::Project => invite.project.is_some() && invite.role.is_some() && !invite.admin,
      InviteKind::Server => invite.project.is_none() && invite.role.is_none(),
      InviteKind::Account => invite.project.is_none() && invite.role.is_none() && !invite.admin,
    };
    let lifetime = invite.expires_at - invite.issued_at;
    let timely = invite.expires_at > now
      && invite.issued_at <= now + SKEW_MS
      && (0..=MAX_LIFETIME_MS + SKEW_MS).contains(&lifetime);
    if !shape || !timely {
      return Err(CloudErrorCode::Invalid.into());
    }
    let project = invite
      .project
      .as_ref()
      .map(|project| {
        Ok::<_, CloudError>((
          input::text(&project.name, MAX_NAME_CHARS)?,
          input::text(&project.address, MAX_ADDRESS_CHARS)?,
        ))
      })
      .transpose()?;
    Ok(Self {
      email: input::email(&invite.email)?,
      inviter: input::text(&invite.inviter, MAX_NAME_CHARS)?,
      project,
    })
  }
}

const fn kind_name(kind: InviteKind) -> &'static str {
  match kind {
    InviteKind::Server => "server",
    InviteKind::Project => "project",
    InviteKind::Account => "account",
  }
}

fn parse_kind(value: &str) -> Option<InviteKind> {
  [InviteKind::Server, InviteKind::Project, InviteKind::Account]
    .into_iter()
    .find(|kind| kind_name(*kind) == value)
}

const fn role_name(role: MemberRole) -> &'static str {
  match role {
    MemberRole::Owner => "owner",
    MemberRole::Member => "member",
  }
}

fn parse_role(value: &str) -> Option<MemberRole> {
  [MemberRole::Owner, MemberRole::Member]
    .into_iter()
    .find(|role| role_name(*role) == value)
}
