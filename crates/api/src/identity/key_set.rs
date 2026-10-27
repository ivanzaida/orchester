use serde::{Deserialize, Serialize};

use super::{CLOCK_SKEW_SECS, IdentityClaims, IdentityError, PublicKey, RootKey, jws};

const DAY_SECS: i64 = 24 * 60 * 60;
/// A key set is valid for 30 days after it was issued; the cloud publishes a new one at least weekly.
pub const KEY_SET_LIFETIME_SECS: i64 = 30 * DAY_SECS;
/// A signing key signs tokens for at most 30 days ...
pub const KEY_SIGNS_SECS: i64 = 30 * DAY_SECS;
/// ... and stays listed at most 60 days longer, so the tokens it signed last still verify.
pub const KEY_LISTED_AFTER_SECS: i64 = 60 * DAY_SECS;
/// The root signs only a statement prepared within the last day: an old statement may list keys since replaced.
pub const STATEMENT_FRESH_SECS: i64 = DAY_SECS;
const TYPE: &str = "orchester-key-set+jws";

/// A signing key as a key set lists it. Times are seconds since the Unix epoch: the key signs tokens issued in
/// `not_before..signs_until`, and tokens it signed verify until `listed_until`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListedKey {
  pub kid: String,
  pub public_key: String,
  pub not_before: i64,
  pub signs_until: i64,
  pub listed_until: i64,
}

/// What the root key signs: the signing keys the cloud lists, and how long the set is valid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeySetStatement {
  pub iss: String,
  pub issued_at: i64,
  pub expires_at: i64,
  pub keys: Vec<ListedKey>,
}

impl KeySetStatement {
  pub fn new(issuer: String, keys: Vec<ListedKey>, now: i64) -> Self {
    Self {
      iss: issuer,
      issued_at: now,
      expires_at: now + KEY_SET_LIFETIME_SECS,
      keys,
    }
  }

  /// At least one key is listed; the set lives at most 30 days; every listed key parses, carries the id its key
  /// derives, and has ordered times within the 30 and 60 days of the rules. Both the signer and every verifier
  /// check this, so even a careless or stolen signer cannot make a long-lived key set that team servers believe.
  fn checked_keys(&self) -> Result<Vec<(ListedKey, PublicKey)>, IdentityError> {
    // Checked: the times come from a signed but untrusted statement, and a wrapped difference would pass.
    let lifetime = self.expires_at.checked_sub(self.issued_at);
    if self.keys.is_empty() || !lifetime.is_some_and(|lifetime| (1..=KEY_SET_LIFETIME_SECS).contains(&lifetime)) {
      return Err(IdentityError::InvalidStatement);
    }
    self
      .keys
      .iter()
      .map(|listed| {
        let key = PublicKey::parse(&listed.public_key)?;
        let signs = listed.signs_until.checked_sub(listed.not_before);
        let listed_after = listed.listed_until.checked_sub(listed.signs_until);
        let within = signs.is_some_and(|signs| (1..=KEY_SIGNS_SECS).contains(&signs))
          && listed_after.is_some_and(|after| (0..=KEY_LISTED_AFTER_SECS).contains(&after));
        if key.kid() != listed.kid || !within {
          return Err(IdentityError::InvalidStatement);
        }
        Ok((listed.clone(), key))
      })
      .collect()
  }
}

impl RootKey {
  /// Signs a statement prepared at most a day before `now` (seconds since the Unix epoch, the signer's clock).
  pub fn sign_key_set(&self, statement: &KeySetStatement, now: i64) -> Result<String, IdentityError> {
    statement.checked_keys()?;
    let fresh = now.saturating_sub(STATEMENT_FRESH_SECS)..=now.saturating_add(CLOCK_SKEW_SECS);
    if !fresh.contains(&statement.issued_at) {
      return Err(IdentityError::StaleStatement);
    }
    let input = jws::signing_input(&jws::Header::new(TYPE, None), statement)?;
    let signature = self.sign(input.as_bytes());
    Ok(jws::finish(&input, &signature))
  }
}

/// A key set whose root signature verified and which had not expired when it was verified.
#[derive(Debug)]
pub struct VerifiedKeySet {
  statement: KeySetStatement,
  keys: Vec<(ListedKey, PublicKey)>,
}

impl VerifiedKeySet {
  pub fn verify(compact: &str, root: &PublicKey, now: i64) -> Result<Self, IdentityError> {
    let compact = jws::decode(compact, TYPE)?;
    root.verify(&compact.signing_input, &compact.signature)?;
    let statement: KeySetStatement = jws::parse(&compact.payload)?;
    let keys = statement.checked_keys()?;
    if statement.issued_at > now.saturating_add(CLOCK_SKEW_SECS) {
      return Err(IdentityError::NotYetValid);
    }
    if now >= statement.expires_at {
      return Err(IdentityError::KeySetExpired);
    }
    Ok(Self { statement, keys })
  }

  pub fn statement(&self) -> &KeySetStatement {
    &self.statement
  }

  /// Verifies an identity token for the team server `audience` at `now` (seconds since the Unix epoch).
  pub fn verify_token(&self, token: &str, audience: &str, now: i64) -> Result<VerifiedToken, IdentityError> {
    self.verify_token_also_from(token, audience, None, now)
  }

  /// [`Self::verify_token`], also taking a token issued by `former`: the issuer an earlier key set of the same root
  /// named, before the cloud moved to another public URL (`docs/cloud-hosting.md` in `ivanzaida/orchester-cloud`,
  /// "Moving the cloud"). The newest key set names the cloud's new URL while tokens issued just before the move still
  /// name the former one.
  pub fn verify_token_also_from(
    &self,
    token: &str,
    audience: &str,
    former: Option<&str>,
    now: i64,
  ) -> Result<VerifiedToken, IdentityError> {
    if now >= self.statement.expires_at {
      return Err(IdentityError::KeySetExpired);
    }
    let compact = jws::decode(token, IdentityClaims::TYPE)?;
    let kid = compact.header.kid.as_deref().ok_or(IdentityError::Malformed)?;
    let (listed, key) = self
      .keys
      .iter()
      .find(|(listed, _)| listed.kid == kid && now < listed.listed_until)
      .ok_or(IdentityError::UnknownKey)?;
    key.verify(&compact.signing_input, &compact.signature)?;
    let claims: IdentityClaims = jws::parse(&compact.payload)?;
    if claims.iss != self.statement.iss && former != Some(claims.iss.as_str()) {
      return Err(IdentityError::WrongIssuer);
    }
    if claims.aud != audience {
      return Err(IdentityError::WrongAudience);
    }
    claims.check_times(listed, now)?;
    Ok(VerifiedToken {
      claims,
      kid: listed.kid.clone(),
    })
  }
}

/// A verified identity token: what it says, and the listed key that signed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedToken {
  pub claims: IdentityClaims,
  pub kid: String,
}
