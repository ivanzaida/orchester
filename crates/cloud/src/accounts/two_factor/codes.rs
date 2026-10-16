//! Checking a two-factor code: six digits from the authenticator app, accepted for the current 30-second step or the
//! one before or after it, each step once; or a recovery code, each once. Every check counts against the account's
//! budget (`budget.rs`), and a used recovery code is mailed to the account's email.

use deadpool_postgres::GenericClient;
use orchester_api::{
  Credential,
  totp::{self, DIGITS},
};
use ring::hmac;

use super::Authenticator;
use crate::{
  Cloud, accounts::rows::account, clock::seconds, db::time, error::CloudError, keys::derived_key, tokens::random_bytes,
};

/// Crockford's base32 in lower case: no `i`, `l`, `o` or `u`, so a code read aloud or typed from paper is not misread.
const RECOVERY_ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";
/// Ten characters, 50 random bits, written `xxxxx-xxxxx`.
const RECOVERY_LENGTH: usize = 10;
const RECOVERY_CODES: usize = 10;
/// Separates the recovery codes' key from the key-encryption key's other uses.
const INFO: &[u8] = b"orchester-cloud recovery codes v1";

/// Keeps recovery codes as an HMAC under a key derived from the key-encryption key: the database alone cannot check a
/// guess against them.
pub(crate) struct RecoveryKey(hmac::Key);

impl RecoveryKey {
  pub fn new(key: &Credential) -> Result<Self, CloudError> {
    derived_key(key, INFO).map(Self)
  }

  fn hash(&self, normalized: &str) -> Vec<u8> {
    hmac::sign(&self.0, normalized.as_bytes()).as_ref().to_vec()
  }
}

/// A code as the person typed it.
enum Code {
  App(String),
  Recovery(String),
  Malformed,
}

/// Spaces and dashes do not matter, nor case; `o` reads as `0` and `i` or `l` as `1`, as Crockford's base32 reads them.
fn read(code: &Credential) -> Code {
  let compact: String = code
    .expose()
    .chars()
    .filter(|character| !character.is_whitespace() && *character != '-')
    .collect();
  if compact.len() == DIGITS && compact.bytes().all(|byte| byte.is_ascii_digit()) {
    return Code::App(compact);
  }
  let normalized: Option<String> = compact
    .chars()
    .map(|character| match character.to_ascii_lowercase() {
      'o' => Some('0'),
      'i' | 'l' => Some('1'),
      other if other.is_ascii() && RECOVERY_ALPHABET.contains(&(other as u8)) => Some(other),
      _ => None,
    })
    .collect();
  match normalized {
    Some(normalized) if normalized.len() == RECOVERY_LENGTH => Code::Recovery(normalized),
    _ => Code::Malformed,
  }
}

/// Ten new recovery codes, `xxxxx-xxxxx`.
pub(super) fn new_recovery_codes() -> Result<Vec<String>, CloudError> {
  (0..RECOVERY_CODES)
    .map(|_| {
      let bytes = random_bytes::<RECOVERY_LENGTH>()?;
      let mut code = String::with_capacity(RECOVERY_LENGTH + 1);
      for (index, byte) in bytes.iter().enumerate() {
        if index == RECOVERY_LENGTH / 2 {
          code.push('-');
        }
        // 256 is a multiple of 32, so every character is equally likely.
        code.push(char::from(RECOVERY_ALPHABET[usize::from(byte & 31)]));
      }
      Ok(code)
    })
    .collect()
}

/// Two codes compared in time that does not depend on where they differ.
fn same(left: &str, right: &str) -> bool {
  left.len() == right.len()
    && left
      .bytes()
      .zip(right.bytes())
      .fold(0, |differ, (a, b)| differ | (a ^ b))
      == 0
}

/// Which kind of code was used.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Used {
  App,
  Recovery,
}

impl Cloud {
  /// Checks `code` for `authenticator` within the account's budget; `recovery` also takes a recovery code. A wrong or
  /// used code is `invalid_code`, audited and counted; a right one is spent and gives its reservation back.
  pub(super) async fn check_code(
    &self,
    client: &impl GenericClient,
    authenticator: &Authenticator,
    code: &Credential,
    recovery: bool,
  ) -> Result<Used, CloudError> {
    let attempt = self.attempt_code(client, authenticator).await?;
    match self.spend_code(client, authenticator, code, recovery).await? {
      Some(used) => {
        self.code_passed(client, authenticator, (attempt, used)).await?;
        Ok(used)
      }
      None => Err(self.code_failed(client, authenticator).await?),
    }
  }

  /// Spends `code` with `client`, which may be a transaction that rolls back to leave it unspent: a code of the app
  /// for a step not used yet, or with `recovery` an unused recovery code. `None` for any other.
  pub(super) async fn spend_code(
    &self,
    client: &impl GenericClient,
    authenticator: &Authenticator,
    code: &Credential,
    recovery: bool,
  ) -> Result<Option<Used>, CloudError> {
    match read(code) {
      Code::App(digits) => self.app_code(client, authenticator, &digits).await,
      Code::Recovery(normalized) if recovery => self.recovery_code(client, authenticator, &normalized).await,
      Code::Recovery(_) | Code::Malformed => Ok(None),
    }
  }

  /// A code of the app for the step before, at or after now, later than the last step accepted, which it becomes.
  async fn app_code(
    &self,
    client: &impl GenericClient,
    authenticator: &Authenticator,
    digits: &str,
  ) -> Result<Option<Used>, CloudError> {
    let now = totp::step(seconds(self.now()));
    let matched = [now.saturating_sub(1), now, now + 1]
      .into_iter()
      .find(|step| same(&totp::code(&authenticator.secret, *step), digits));
    let Some(step) = matched else {
      return Ok(None);
    };
    let step = i64::try_from(step).map_err(|_| CloudError::StoredData)?;
    let taken = client
      .execute(
        "UPDATE two_factor SET last_step = $2 WHERE id = $1 AND (last_step IS NULL OR last_step < $2)",
        &[&authenticator.id, &step],
      )
      .await?;
    Ok((taken == 1).then_some(Used::App))
  }

  /// An unused recovery code of the authenticator, which is then used.
  async fn recovery_code(
    &self,
    client: &impl GenericClient,
    authenticator: &Authenticator,
    normalized: &str,
  ) -> Result<Option<Used>, CloudError> {
    let used = client
      .execute(
        "UPDATE recovery_codes SET used_at = $3 WHERE two_factor = $1 AND code_hash = $2 AND used_at IS NULL",
        &[
          &authenticator.id,
          &self.inner.recovery.hash(normalized),
          &time(self.now()),
        ],
      )
      .await?;
    Ok((used == 1).then_some(Used::Recovery))
  }

  /// Keeps `codes` as the authenticator's recovery codes, in the caller's transaction.
  pub(super) async fn store_recovery_codes(
    &self,
    client: &impl GenericClient,
    authenticator: &Authenticator,
    codes: &[String],
  ) -> Result<(), CloudError> {
    for code in codes {
      let normalized = code.replace('-', "");
      client
        .execute(
          "INSERT INTO recovery_codes (two_factor, code_hash) VALUES ($1, $2)",
          &[&authenticator.id, &self.inner.recovery.hash(&normalized)],
        )
        .await?;
    }
    Ok(())
  }
}

/// The authenticator's recovery codes not used yet.
pub(super) async fn recovery_codes_left(
  client: &impl GenericClient,
  authenticator: &Authenticator,
) -> Result<u32, CloudError> {
  let row = client
    .query_one(
      "SELECT count(*) FROM recovery_codes WHERE two_factor = $1 AND used_at IS NULL",
      &[&authenticator.id],
    )
    .await?;
  u32::try_from(row.try_get::<_, i64>(0)?).map_err(|_| CloudError::StoredData)
}

pub(super) async fn account_email(
  client: &impl GenericClient,
  id: orchester_core::id::CloudAccountId,
) -> Result<String, CloudError> {
  Ok(account(client, id).await?.account.email)
}
