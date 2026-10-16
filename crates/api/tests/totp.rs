//! The cloud's two-factor codes against RFC 6238's SHA-1 test vectors, the base32 of its secrets, and the identity
//! token's two-factor claim.

use orchester_api::{
  cloud::{CloudPlatform, DeviceInfo},
  identity::IdentityClaims,
  totp,
};

/// RFC 6238, appendix B: the SHA-1 secret is the ASCII of "12345678901234567890"; its eight-digit codes end in these
/// six digits.
#[test]
fn totp_codes_match_the_rfc_vectors() {
  let secret = b"12345678901234567890";
  let vectors = [
    (59, "287082"),
    (1_111_111_109, "081804"),
    (1_111_111_111, "050471"),
    (1_234_567_890, "005924"),
    (2_000_000_000, "279037"),
    (20_000_000_000, "353130"),
  ];
  for (seconds, code) in vectors {
    assert_eq!(totp::code(secret, totp::step(seconds)), code, "at {seconds}");
  }
  assert_eq!(totp::step(29), 0);
  assert_eq!(totp::step(30), 1);
  assert_eq!(totp::step(-5), 0, "a time before 1970 is the first step");
}

#[test]
fn totp_secrets_round_trip_through_base32() {
  let secret = b"12345678901234567890";
  let encoded = totp::encode_secret(secret);
  assert_eq!(encoded, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
  assert_eq!(totp::decode_secret(&encoded).as_deref(), Some(secret.as_slice()));
  assert_eq!(
    totp::decode_secret("gezd gnbv gy3t qojq gezd gnbv gy3t qojq").as_deref(),
    Some(secret.as_slice()),
    "case and spaces do not matter"
  );
  assert_eq!(totp::encode_secret(b"f"), "MY");
  assert_eq!(totp::decode_secret("MY======").as_deref(), Some(b"f".as_slice()));
  assert_eq!(totp::decode_secret("M1"), None, "1 is not base32");
}

/// The claim is left out when false, so a token without it (from a cloud before two-factor authentication) reads as
/// not two-factor, and one with it says so.
#[test]
fn totp_identity_claim_is_absent_unless_true() {
  let claims = |mfa: bool| IdentityClaims {
    iss: "https://cloud.test".into(),
    aud: "server".into(),
    sub: "0191c7c0-0000-7000-8000-000000000b0b".parse().expect("account id"),
    email: "ada@example.test".into(),
    name: "Ada".into(),
    device: DeviceInfo {
      name: "Ada's laptop".into(),
      platform: CloudPlatform::Macos,
    },
    invite: None,
    purpose: None,
    mfa,
    jti: "jti".into(),
    iat: 1,
    exp: 2,
  };
  let without = serde_json::to_value(claims(false)).expect("JSON");
  assert!(without.get("mfa").is_none());
  let read: IdentityClaims = serde_json::from_value(without).expect("claims");
  assert!(!read.mfa);
  let with = serde_json::to_value(claims(true)).expect("JSON");
  assert_eq!(with["mfa"], serde_json::json!(true));
}
