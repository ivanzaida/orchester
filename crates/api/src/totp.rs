//! Time-based one-time codes (RFC 6238) as Orchester Cloud checks them for two-factor authentication: HMAC-SHA-1, six
//! digits, a 30-second step (`docs/cloud-api.md`, "Two-factor authentication"). The secret is written in base32
//! (RFC 4648, no padding), as authenticator apps read it from an `otpauth://` link. The cloud checks codes with it;
//! the tests use it as the person's authenticator app.

use ring::hmac;

/// Seconds one code is valid for.
pub const STEP_SECS: i64 = 30;
/// The digits of a code.
pub const DIGITS: usize = 6;

const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// The step `unix_secs` falls in: codes change at each step. A time before 1970 is step 0.
pub fn step(unix_secs: i64) -> u64 {
  unix_secs.max(0).div_euclid(STEP_SECS).unsigned_abs()
}

/// The code of `step` for `secret` (RFC 4226's dynamic truncation), zero-padded to six digits.
pub fn code(secret: &[u8], step: u64) -> String {
  let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, secret);
  let mac = hmac::sign(&key, &step.to_be_bytes());
  let mac = mac.as_ref();
  let offset = usize::from(mac[mac.len() - 1] & 0x0f);
  let binary = u32::from_be_bytes([mac[offset] & 0x7f, mac[offset + 1], mac[offset + 2], mac[offset + 3]]);
  format!("{:06}", binary % 1_000_000)
}

/// `bytes` in base32, upper case, without padding.
pub fn encode_secret(bytes: &[u8]) -> String {
  let mut text = String::with_capacity(bytes.len().div_ceil(5) * 8);
  let (mut buffer, mut bits) = (0u32, 0u32);
  for byte in bytes {
    buffer = (buffer << 8) | u32::from(*byte);
    bits += 8;
    while bits >= 5 {
      bits -= 5;
      text.push(char::from(BASE32[((buffer >> bits) & 31) as usize]));
    }
  }
  if bits > 0 {
    text.push(char::from(BASE32[((buffer << (5 - bits)) & 31) as usize]));
  }
  text
}

/// The bytes of a base32 secret as an authenticator app takes it: case, spaces and padding do not matter; `None` for
/// any other character.
pub fn decode_secret(text: &str) -> Option<Vec<u8>> {
  let mut bytes = Vec::new();
  let (mut buffer, mut bits) = (0u32, 0u32);
  for character in text.chars().filter(|c| !c.is_whitespace() && *c != '=') {
    let upper = character.to_ascii_uppercase();
    let value = BASE32.iter().position(|letter| char::from(*letter) == upper)?;
    buffer = (buffer << 5) | u32::try_from(value).ok()?;
    bits += 5;
    if bits >= 8 {
      bits -= 8;
      bytes.push(u8::try_from((buffer >> bits) & 0xff).ok()?);
    }
  }
  Some(bytes)
}
