//! A team server as the cloud sees it: an id and an Ed25519 key that signs its link statement and its invites, in the
//! shapes `orchester_api::cloud` defines (and `crates/server` signs; its test checks the same contract).

use orchester_api::{
  InviteKind, MemberRole,
  cloud::{INVITE_STATEMENT_TYPE, InviteStatement, InvitedProject, LINK_STATEMENT_TYPE, LinkStatement},
  identity::{PrivateKey, SigningKey},
};
use orchester_core::id::{InviteId, TeamServerId};

use super::{DAY, remember};

pub const FINGERPRINT: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";

pub struct TeamServer {
  pub id: TeamServerId,
  pub address: String,
  key: SigningKey,
}

impl TeamServer {
  pub fn generate() -> Self {
    Self::with_id(TeamServerId::generate())
  }

  pub fn with_id(id: TeamServerId) -> Self {
    let private = PrivateKey::generate().expect("server key");
    Self {
      id,
      address: "https://team.example.test:8740".into(),
      key: SigningKey::new(&private).expect("signing key"),
    }
  }

  pub fn public_key(&self) -> String {
    self.key.public_key().encoded()
  }

  /// The link statement the server signs at setup, issued at `now_ms`.
  pub fn link(&self, name: &str, now_ms: i64) -> String {
    self.sign_link(&LinkStatement {
      server: self.id,
      name: name.into(),
      address: self.address.clone(),
      public_key: self.public_key(),
      fingerprint: Some(FINGERPRINT.into()),
      issued_at: now_ms / 1000,
    })
  }

  pub fn sign_link(&self, statement: &LinkStatement) -> String {
    self
      .key
      .sign_statement(LINK_STATEMENT_TYPE, statement)
      .expect("sign the link")
  }

  /// A project invite to `email` from Tomas Berg, open 7 days from `now_ms`.
  pub fn project_invite(&self, email: &str, now_ms: i64) -> InviteStatement {
    remember(email);
    InviteStatement {
      id: InviteId::generate(),
      server: self.id,
      kind: InviteKind::Project,
      email: email.into(),
      project: Some(InvitedProject {
        address: format!("{}/kontur", self.address),
        name: "Kontur".into(),
      }),
      role: Some(MemberRole::Member),
      admin: false,
      inviter: "Tomas Berg".into(),
      expires_at: now_ms + 7 * DAY,
      issued_at: now_ms,
    }
  }

  pub fn sign_invite(&self, invite: &InviteStatement) -> String {
    let signed = self
      .key
      .sign_statement(INVITE_STATEMENT_TYPE, invite)
      .expect("sign the invite");
    // The payload holds the invitee's email in base64url; the log must not hold it either.
    remember(&signed);
    signed
  }
}
