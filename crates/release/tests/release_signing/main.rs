//! CI's signing and upload of release files (`docs/stable-build.md`, "Releases from CI"): the key signs the contract's
//! fixed vector, a throwaway key's signature verifies with the accepted keys, and the upload against a stand-in cloud.
//!
//! `cargo test -p orchester-release --test release_signing release_signing`

mod stand;
mod upload;

use orchester_api::releases::{
  AcceptedReleaseKeys, FileDigest, FileSignature, ReleaseArch, ReleaseFileName, ReleasePlatform, ReleaseSigningKey,
  ReleaseVersion, SignatureError, SignedFile, SigningKeyError,
};
use orchester_release::{PublishError, ReleaseFile};

/// RFC 8032's first Ed25519 test key, the release key of `orchester-api`'s contract tests.
const SEED: &str = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
const PUBLIC: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
const KEY_ID: &str = "a594e17b5a19c648";
/// [`SEED`]'s signature of the contract's vector (`crates/api/tests/releases/main.rs`).
const SIGNATURE: &str = "964d975ae3bc236c01e2496a3506d756c98e4c5ff549210838e4e8c476e3c44982bbd096c77bef09bd20bebf1b32652f1f40ef8bcd8a432d5bcac2cb82f01e04";

fn version(text: &str) -> ReleaseVersion {
  ReleaseVersion::parse(text).expect("a version")
}

fn file_name(text: &str) -> ReleaseFileName {
  ReleaseFileName::parse(text).expect("a file name")
}

fn throwaway() -> ReleaseSigningKey {
  ReleaseSigningKey::generate().expect("a key").0
}

#[test]
fn release_signing_key_signs_the_contracts_vector() {
  let key = ReleaseSigningKey::parse(SEED).expect("the test key");
  assert_eq!(key.public_key().to_hex(), PUBLIC);
  assert_eq!(key.id().to_hex(), KEY_ID);
  let vector = SignedFile {
    version: version("0.4.0"),
    platform: ReleasePlatform::Windows,
    arch: ReleaseArch::X86_64,
    name: file_name("Orchester-0.4.0-windows-x86_64.msi"),
    size: 48_213_504,
    sha256: FileDigest::parse("1055256c88ce265e44b7f378877ffe1c8e85c7670c4fa552f69f2e691a682490").expect("a digest"),
  };
  assert_eq!(key.sign(&vector), FileSignature::parse(SIGNATURE).expect("a signature"));
}

#[test]
fn release_signing_declares_what_the_accepted_keys_verify() {
  let key = throwaway();
  let bytes = b"an installer".to_vec();
  let release = version("0.5.0-beta.1");
  let file = ReleaseFile::sign(
    file_name("Orchester-0.5.0-beta.1-setup.exe"),
    bytes.clone(),
    &release,
    ReleasePlatform::Windows,
    ReleaseArch::X86_64,
    &key,
  );
  let declared = file.declared();
  assert_eq!(declared.size, bytes.len() as u64);
  assert_eq!(declared.sha256, FileDigest::of(&bytes));
  assert_eq!(declared.key_id, key.id());

  let accepted = AcceptedReleaseKeys::new(vec![key.public_key()]);
  let signed = declared.signed(&release);
  assert_eq!(accepted.verify(&signed, declared.key_id, &declared.signature), Ok(()));
  let other = AcceptedReleaseKeys::new(vec![throwaway().public_key()]);
  assert_eq!(
    other.verify(&signed, declared.key_id, &declared.signature),
    Err(SignatureError::UnknownKey)
  );
  let moved = declared.signed(&version("0.5.0"));
  assert_eq!(
    accepted.verify(&moved, declared.key_id, &declared.signature),
    Err(SignatureError::BadSignature)
  );
}

#[test]
fn release_signing_key_never_shows_its_seed() {
  let key = ReleaseSigningKey::parse(SEED).expect("the test key");
  let shown = format!("{key:?}");
  assert_eq!(shown, format!("ReleaseSigningKey({KEY_ID})"));
  assert!(!shown.contains(SEED));
  let (generated, seed) = ReleaseSigningKey::generate().expect("a key");
  assert!(!format!("{generated:?}").contains(seed.expose()));
  assert_eq!(format!("{seed:?}"), "SecretValue(<redacted>)");
  assert_eq!(
    ReleaseSigningKey::parse(seed.expose()).expect("its seed").id(),
    generated.id()
  );
}

#[test]
fn release_signing_refuses_a_seed_in_another_spelling() {
  for seed in [&SEED.to_uppercase(), &SEED[..62], "", &format!("{SEED}00")] {
    assert_eq!(ReleaseSigningKey::parse(seed).err(), Some(SigningKeyError::Format));
  }
}

#[test]
fn release_signing_reads_a_file_under_its_own_name() {
  let folder = tempfile::tempdir().expect("a folder");
  let path = folder.path().join("Orchester-0.5.0-windows-x64.zip");
  std::fs::write(&path, b"a zip").expect("written");
  let key = throwaway();
  let file = ReleaseFile::read(
    &path,
    &version("0.5.0"),
    ReleasePlatform::Windows,
    ReleaseArch::X86_64,
    &key,
  )
  .expect("read");
  assert_eq!(file.name(), &file_name("Orchester-0.5.0-windows-x64.zip"));
  assert_eq!(file.declared().sha256, FileDigest::of(b"a zip"));

  let unnamed = folder.path().join("Orchester 0.5.0.zip");
  std::fs::write(&unnamed, b"a zip").expect("written");
  let refused = ReleaseFile::read(
    &unnamed,
    &version("0.5.0"),
    ReleasePlatform::Windows,
    ReleaseArch::X86_64,
    &key,
  );
  assert!(matches!(refused, Err(PublishError::Name { .. })));
  let missing = folder.path().join("Orchester-0.5.0-setup.exe");
  let refused = ReleaseFile::read(
    &missing,
    &version("0.5.0"),
    ReleasePlatform::Windows,
    ReleaseArch::X86_64,
    &key,
  );
  assert!(matches!(refused, Err(PublishError::Read { .. })));
}
