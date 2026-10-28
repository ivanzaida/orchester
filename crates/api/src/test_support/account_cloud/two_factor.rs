//! Two-factor authentication of the stand-in cloud (`docs/cloud-api.md`, "Two-factor authentication"), for Bohdan's
//! account; Olena has it off. Bohdan has it on, with [`SECRET`] in his authenticator app and [`RECOVERY_CODES`], and a
//! password sign-in counts as one with a code, as the fixtures always had it, until the test asks for codes at
//! sign-in: then the password answers a challenge, and a code of the app's (step before, now and after, each once) or
//! an unused recovery code finishes it. A challenge takes five codes; the test can end every challenge, make the codes
//! answer `429`, and make this computer's session one without a code.

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::totp;

/// The secret of Bohdan's authenticator app.
pub const SECRET: &[u8; 20] = b"orchester stand-in 2";
/// Bohdan's recovery codes, as the cloud showed them once.
pub const RECOVERY_CODES: [&str; 10] = [
  "a2b3c-d4e5f",
  "g6h7j-k8m9n",
  "p0q1r-s2t3v",
  "w4x5y-z6a7b",
  "c8d9e-f0g1h",
  "j2k3m-n4p5q",
  "r6s7t-v8w9x",
  "y0z1a-b2c3d",
  "e4f5g-h6j7k",
  "m8n9p-q0r1s",
];
/// The codes a challenge takes, right or wrong.
const CHALLENGE_CODES: u32 = 5;
/// How long a challenge lasts, in milliseconds.
const CHALLENGE_LIFE: i64 = 5 * 60 * 1000;

/// A sign-in the password started: its device, the codes tried, and when it ends (milliseconds).
struct Challenge {
  device: Value,
  tried: u32,
  expires_at: i64,
}

/// What the stand-in holds of Bohdan's two-factor authentication.
pub(super) struct Factors {
  pub enabled: bool,
  /// This computer signed in with a code, or as if it had.
  pub this_session: bool,
  /// The password answers a challenge.
  pub codes_at_sign_in: bool,
  /// Every code check answers `429`.
  pub limited: bool,
  /// The last step of the app's codes accepted: it and every earlier one are refused.
  last_step: Option<u64>,
  used_recovery: Vec<usize>,
  challenges: HashMap<String, Challenge>,
  issued: u64,
}

impl Default for Factors {
  fn default() -> Self {
    Self {
      enabled: true,
      this_session: true,
      codes_at_sign_in: false,
      limited: false,
      last_step: None,
      used_recovery: Vec::new(),
      challenges: HashMap::new(),
      issued: 0,
    }
  }
}

/// How a code check ended badly: the status line and the cloud's error code.
pub(super) type Refusal = (&'static str, &'static str);

impl Factors {
  /// `GET /me`'s `two_factor` for the account `bohdan` or another.
  pub fn status(&self, bohdan: bool) -> Value {
    let on = bohdan && self.enabled;
    let left = if on { self.recovery_left() } else { 0 };
    json!({"enabled": on, "enabled_at": on.then_some(1), "recovery_codes_left": left,
      "this_session": on && self.this_session})
  }

  /// Whether the device of `bohdan` or another account is a two-factor session.
  pub fn session(&self, bohdan: bool) -> bool {
    bohdan && self.enabled && self.this_session
  }

  /// Why the cloud refuses a token that accepts an invite, for `bohdan` or another account.
  pub fn refusal(&self, bohdan: bool) -> Option<&'static str> {
    if !bohdan || !self.enabled {
      return Some("mfa_required");
    }
    (!self.this_session).then_some("session_without_mfa")
  }

  /// The password was right for Bohdan's `device`: a challenge when sign-ins take a code, at `now` (milliseconds).
  pub fn challenge(&mut self, device: &Value, now: i64) -> Option<Value> {
    if !(self.enabled && self.codes_at_sign_in) {
      self.this_session = self.enabled;
      return None;
    }
    self.issued += 1;
    let challenge = format!("orcm_{:064x}", self.issued);
    let expires_at = now + CHALLENGE_LIFE;
    self.challenges.insert(
      challenge.clone(),
      Challenge {
        device: device.clone(),
        tried: 0,
        expires_at,
      },
    );
    Some(json!({"challenge": challenge, "expires_at": expires_at}))
  }

  /// Every challenge ends, as when five minutes passed.
  pub fn end_challenges(&mut self) {
    self.challenges.clear();
  }

  /// Marks the first `count` recovery codes used.
  pub fn use_recovery(&mut self, count: usize) {
    self.used_recovery = (0..count.min(RECOVERY_CODES.len())).collect();
  }

  /// `POST /auth/sessions/two-factor` with `body` at `now` (milliseconds): the device the challenge was for and the
  /// recovery codes left, or the refusal.
  pub fn finish(&mut self, body: &str, now: i64) -> Result<(Value, u32), Refusal> {
    let request: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let challenge = request["challenge"].as_str().unwrap_or_default().to_owned();
    let code = request["code"].as_str().unwrap_or_default().to_owned();
    let expired = ("401 Unauthorized", "challenge_expired");
    let open = self
      .challenges
      .get_mut(&challenge)
      .filter(|open| open.expires_at > now && open.tried < CHALLENGE_CODES)
      .ok_or(expired)?;
    if self.limited {
      return Err(("429 Too Many Requests", "rate_limited"));
    }
    open.tried += 1;
    let (tried, device) = (open.tried, open.device.clone());
    if self.check(&code, now / 1000) {
      self.challenges.remove(&challenge);
      self.this_session = true;
      return Ok((device, self.recovery_left()));
    }
    if tried >= CHALLENGE_CODES {
      self.challenges.remove(&challenge);
    }
    Err(("401 Unauthorized", "invalid_code"))
  }

  /// Whether `code` is a code of the app for a step not used yet around `now` (seconds), or an unused recovery code;
  /// either is spent.
  fn check(&mut self, code: &str, now: i64) -> bool {
    let step = totp::step(now);
    let fresh = (step.saturating_sub(1)..=step + 1)
      .filter(|candidate| self.last_step.is_none_or(|last| *candidate > last))
      .find(|candidate| totp::code(SECRET, *candidate) == code);
    if let Some(step) = fresh {
      self.last_step = Some(step);
      return true;
    }
    let typed = normalized(code);
    let found = RECOVERY_CODES
      .iter()
      .position(|recovery| normalized(recovery) == typed)
      .filter(|index| !self.used_recovery.contains(index));
    found.is_some_and(|index| {
      self.used_recovery.push(index);
      true
    })
  }

  /// The next code of the app the stand-in takes at `now` (seconds): the current step's, or the one after when that was
  /// used. Never the step before, which a new half minute starting before the code arrives would leave behind.
  pub fn next_code(&self, now: i64) -> String {
    let step = totp::step(now);
    let next = self.last_step.map_or(step, |last| (last + 1).max(step));
    assert!(next <= step + 1, "the codes of this half minute were used");
    totp::code(SECRET, next)
  }

  fn recovery_left(&self) -> u32 {
    u32::try_from(RECOVERY_CODES.len() - self.used_recovery.len()).expect("ten codes fit")
  }
}

/// A recovery code as the cloud compares it: lower case, without spaces and dashes, `o`, `i` and `l` read as digits.
fn normalized(code: &str) -> String {
  code
    .chars()
    .filter(|character| !character.is_whitespace() && *character != '-')
    .map(|character| match character.to_ascii_lowercase() {
      'o' => '0',
      'i' | 'l' => '1',
      other => other,
    })
    .collect()
}
