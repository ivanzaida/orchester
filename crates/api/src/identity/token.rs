use orchester_core::id::CloudAccountId;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

use super::{CLOCK_SKEW_SECS, IdentityError, ListedKey, SigningKey, jws};
use crate::{
  Credential,
  cloud::{DeviceInfo, TokenPurpose},
};

/// An identity token is valid for five minutes after it was issued.
pub const TOKEN_LIFETIME_SECS: i64 = 5 * 60;

/// What an identity token says. Times are seconds since the Unix epoch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityClaims {
  pub iss: String,
  /// The id of the one team server the token is for.
  pub aud: String,
  pub sub: CloudAccountId,
  /// The account's confirmed email.
  pub email: String,
  pub name: String,
  pub device: DeviceInfo,
  /// The invite the token accepts, when it was issued for one (`docs/architecture/server.md`, "Identity tokens and
  /// keys"); the team server then admits the verified email to that invite.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub invite: Option<String>,
  /// What the token is for besides joining with a device: signing in to the server admin, or linking the server.
  /// A team server accepts a token only for the use it was issued for.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub purpose: Option<TokenPurpose>,
  /// The cloud device the token was issued to signed in with two-factor authentication, or turned it on itself.
  /// Absent means not: a team server lets an admin send invites by email, and admits an invite's token, only with it.
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub mfa: bool,
  /// Unique per token; a team server accepts each once.
  pub jti: String,
  pub iat: i64,
  pub exp: i64,
}

/// Who a token is about and for which team server; `IdentityClaims::new` adds the times and a fresh `jti`.
pub struct TokenSubject {
  pub issuer: String,
  pub audience: String,
  pub account: CloudAccountId,
  pub email: String,
  pub name: String,
  pub device: DeviceInfo,
  pub invite: Option<String>,
  pub purpose: Option<TokenPurpose>,
  pub mfa: bool,
}

impl IdentityClaims {
  pub(super) const TYPE: &'static str = "JWT";

  pub fn new(subject: TokenSubject, now: i64) -> Result<Self, IdentityError> {
    let mut jti = [0u8; 16];
    SystemRandom::new().fill(&mut jti).map_err(|_| IdentityError::Random)?;
    Ok(Self {
      iss: subject.issuer,
      aud: subject.audience,
      sub: subject.account,
      email: subject.email,
      name: subject.name,
      device: subject.device,
      invite: subject.invite,
      purpose: subject.purpose,
      mfa: subject.mfa,
      jti: jws::encode(&jti),
      iat: now,
      exp: now + TOKEN_LIFETIME_SECS,
    })
  }

  pub(super) fn check_times(&self, key: &ListedKey, now: i64) -> Result<(), IdentityError> {
    let lifetime = self.exp.checked_sub(self.iat);
    if !lifetime.is_some_and(|lifetime| (1..=TOKEN_LIFETIME_SECS).contains(&lifetime)) {
      return Err(IdentityError::Malformed);
    }
    if self.iat < key.not_before || self.iat >= key.signs_until {
      return Err(IdentityError::KeyNotValidAtTime);
    }
    if self.iat > now.saturating_add(CLOCK_SKEW_SECS) {
      return Err(IdentityError::NotYetValid);
    }
    if now >= self.exp.saturating_add(CLOCK_SKEW_SECS) {
      return Err(IdentityError::Expired);
    }
    Ok(())
  }
}

impl SigningKey {
  pub fn sign_token(&self, claims: &IdentityClaims) -> Result<Credential, IdentityError> {
    self.sign_statement(IdentityClaims::TYPE, claims).map(Credential::new)
  }

  /// `statement` as a compact JWS of the type `typ` whose header names this key: what a team server signs for the
  /// cloud with its own key (its link statement and invites).
  pub fn sign_statement(&self, typ: &str, statement: &impl Serialize) -> Result<String, IdentityError> {
    let input = jws::signing_input(&jws::Header::new(typ, Some(self.kid())), statement)?;
    let signature = self.sign(input.as_bytes());
    Ok(jws::finish(&input, &signature))
  }
}
