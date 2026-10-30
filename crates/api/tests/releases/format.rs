//! The signed message, the key ids, signatures and the rollout hash, against vectors computed with Python's hashlib
//! and the `cryptography` package.

use orchester_api::releases::{
  AcceptedReleaseKeys, FileDigest, FileSignature, ReleaseFormatError, ReleaseKeyId, ReleasePublicKey, RolloutPercent,
  SignatureError, rollout_bucket,
};
use orchester_core::id::InstallationId;
use ring::signature::Ed25519KeyPair;

use super::{DIGEST, KEY_ID, PUBLIC, SEED, SIGNATURE, vector, version};

const INSTALL: &str = "0192f4d5-7c3a-7b2e-9a41-5d6c7e8f9a0b";

fn bytes<const N: usize>(text: &str) -> [u8; N] {
  let mut bytes = [0_u8; N];
  for (index, byte) in bytes.iter_mut().enumerate() {
    *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).expect("hex");
  }
  bytes
}

fn keys() -> AcceptedReleaseKeys {
  AcceptedReleaseKeys::new(vec![ReleasePublicKey::parse(PUBLIC).expect("a key")])
}

fn signature() -> FileSignature {
  FileSignature::parse(SIGNATURE).expect("a signature")
}

fn key_id() -> ReleaseKeyId {
  ReleaseKeyId::parse(KEY_ID).expect("a key id")
}

#[test]
fn releases_signed_message_has_the_exact_bytes() {
  let expected = format!(
    "orchester-release-file-v1\nversion=0.4.0\nplatform=windows\narch=x86_64\nname=Orchester-0.4.0-windows-x86_64.msi\nsize=48213504\nsha256={DIGEST}\n"
  );
  assert_eq!(vector().message(), expected.as_bytes());
}

#[test]
fn releases_digest_is_the_sha256_of_the_bytes() {
  assert_eq!(FileDigest::of(b"orchester release vector").to_hex(), DIGEST);
}

#[test]
fn releases_key_id_is_the_labelled_hash_of_the_key() {
  let key = ReleasePublicKey::parse(PUBLIC).expect("a key");
  assert_eq!(key.id().to_hex(), KEY_ID);
  assert_eq!(ReleaseKeyId::of(&bytes(PUBLIC)), key.id());
}

#[test]
fn releases_signature_of_the_vector_is_the_fixed_one() {
  let pair = Ed25519KeyPair::from_seed_and_public_key(&bytes::<32>(SEED), &bytes::<32>(PUBLIC)).expect("a key pair");
  assert_eq!(pair.sign(&vector().message()).as_ref(), signature().as_bytes());
}

#[test]
fn releases_accepted_key_verifies_the_vector() {
  assert_eq!(keys().verify(&vector(), key_id(), &signature()), Ok(()));
}

#[test]
fn releases_signature_does_not_verify_another_file() {
  let mut sized = vector();
  sized.size += 1;
  assert_eq!(
    keys().verify(&sized, key_id(), &signature()),
    Err(SignatureError::BadSignature)
  );
  let mut renamed = vector();
  renamed.version = version("0.4.1");
  assert_eq!(
    keys().verify(&renamed, key_id(), &signature()),
    Err(SignatureError::BadSignature)
  );
}

#[test]
fn releases_signature_by_a_key_not_on_the_list_is_refused() {
  let other = ReleaseKeyId::of(&[7; 32]);
  assert_eq!(
    keys().verify(&vector(), other, &signature()),
    Err(SignatureError::UnknownKey)
  );
  let none = AcceptedReleaseKeys::new(Vec::new());
  assert_eq!(
    none.verify(&vector(), key_id(), &signature()),
    Err(SignatureError::UnknownKey)
  );
}

#[test]
fn releases_hex_values_have_one_spelling() {
  assert_eq!(
    FileDigest::parse(&DIGEST.to_uppercase()),
    Err(ReleaseFormatError::Digest)
  );
  assert_eq!(FileDigest::parse(&DIGEST[..62]), Err(ReleaseFormatError::Digest));
  assert_eq!(ReleaseKeyId::parse("A594E17B5A19C648"), Err(ReleaseFormatError::KeyId));
  assert_eq!(
    FileSignature::parse(&SIGNATURE[2..]),
    Err(ReleaseFormatError::Signature)
  );
  assert_eq!(ReleasePublicKey::parse("zz"), Err(ReleaseFormatError::PublicKey));
}

#[test]
fn releases_rollout_bucket_matches_the_vectors() {
  let install: InstallationId = INSTALL.parse().expect("an install id");
  assert_eq!(rollout_bucket(install, &version("0.4.0")), 39);
  assert_eq!(rollout_bucket(install, &version("0.4.1")), 59);
  assert_eq!(rollout_bucket(install, &version("0.5.0-beta.1")), 47);
}

#[test]
fn releases_rollout_reaches_buckets_below_the_percentage() {
  let install: InstallationId = INSTALL.parse().expect("an install id");
  let at = |percent| RolloutPercent::new(percent).expect("a percentage");
  assert!(!at(39).includes(install, &version("0.4.0")));
  assert!(at(40).includes(install, &version("0.4.0")));
  assert!(!at(0).includes(install, &version("0.4.0")));
  assert!(RolloutPercent::ALL.includes(install, &version("0.4.0")));
  assert_eq!(RolloutPercent::new(101), Err(ReleaseFormatError::Percent));
}
