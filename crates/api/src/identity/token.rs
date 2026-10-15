use orchester_core::id::CloudAccountId;
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};

use super::{CLOCK_SKEW_SECS, IdentityError, ListedKey, SigningKey, jws};
use crate::{Credential, cloud::DeviceInfo};

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
      jti: jws::encode(&jti),
      iat: now,
      exp: now + TOKEN_LIFETIME_SECS,
    })
  }

  pub(super) fn check_times(&self, key: &ListedKey, now: i64) -> Result<(), IdentityError> {
    if self.exp <= self.iat || self.exp - self.iat > TOKEN_LIFETIME_SECS {
      return Err(IdentityError::Malformed);
    }
    if self.iat < key.not_before || self.iat >= key.signs_until {
      return Err(IdentityError::KeyNotValidAtTime);
    }
    if self.iat > now + CLOCK_SKEW_SECS {
      return Err(IdentityError::NotYetValid);
    }
    if now >= self.exp + CLOCK_SKEW_SECS {
      return Err(IdentityError::Expired);
    }
    Ok(())
  }
}

impl SigningKey {
  pub fn sign_token(&self, claims: &IdentityClaims) -> Result<Credential, IdentityError> {
    let input = jws::signing_input(&jws::Header::new(IdentityClaims::TYPE, Some(self.kid())), claims)?;
    let signature = self.sign(input.as_bytes());
    Ok(Credential::new(jws::finish(&input, &signature)))
  }
}
