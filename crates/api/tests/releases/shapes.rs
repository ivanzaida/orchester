//! The JSON of each release shape of the update check and CI's upload, written and read back.

use std::fmt::Debug;

use orchester_api::releases::{
  Channel, DeclaredFile, FileDigest, FileResult, FileSignature, FileUpload, FileVerdict, NewRelease, OfferedFile,
  OfferedRelease, Rejection, ReleaseArch, ReleaseKeyId, ReleasePlatform, UpdateCheck, UpdateCheckAnswer, Verification,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use super::{DIGEST, KEY_ID, SIGNATURE, file_name, version};

pub(super) const NAME: &str = "Orchester-0.4.1-windows-x86_64.msi";

/// `value` is written as `expected` and read back as itself.
pub(super) fn round_trip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T, expected: &Value) {
  assert_eq!(&serde_json::to_value(value).expect("written"), expected);
  assert_eq!(&serde_json::from_value::<T>(expected.clone()).expect("read"), value);
}

pub(super) fn digest() -> FileDigest {
  FileDigest::parse(DIGEST).expect("a digest")
}

pub(super) fn key_id() -> ReleaseKeyId {
  ReleaseKeyId::parse(KEY_ID).expect("a key id")
}

fn signature() -> FileSignature {
  FileSignature::parse(SIGNATURE).expect("a signature")
}

#[test]
fn releases_update_check_query_names_every_field() {
  let check = UpdateCheck {
    version: version("0.4.0"),
    platform: ReleasePlatform::Macos,
    arch: ReleaseArch::Aarch64,
    channel: Channel::Beta,
    install_id: "0192f4d5-7c3a-7b2e-9a41-5d6c7e8f9a0b".parse().expect("an id"),
  };
  let query: Vec<(&str, String)> = check.query().into_iter().collect();
  let expected = [
    ("version", "0.4.0"),
    ("platform", "macos"),
    ("arch", "aarch64"),
    ("channel", "beta"),
    ("install_id", "0192f4d5-7c3a-7b2e-9a41-5d6c7e8f9a0b"),
  ];
  assert_eq!(query, expected.map(|(key, value)| (key, value.to_owned())));
}

#[test]
fn releases_update_check_answers_nothing_newer_as_null() {
  round_trip(&UpdateCheckAnswer { update: None }, &json!({ "update": null }));
}

#[test]
fn releases_update_check_answers_the_offered_release() {
  let answer = UpdateCheckAnswer {
    update: Some(OfferedRelease {
      version: version("0.4.1"),
      channel: Channel::Stable,
      notes: "## Fixed\n- Crash on start".to_owned(),
      critical: true,
      published_at: 1_792_147_957_042,
      files: vec![OfferedFile {
        name: file_name(NAME),
        platform: ReleasePlatform::Windows,
        arch: ReleaseArch::X86_64,
        url: format!("https://api.orchester.cloud/v1/releases/0.4.1/files/{NAME}"),
        size: 48_213_504,
        sha256: digest(),
        key_id: key_id(),
        signature: signature(),
      }],
    }),
  };
  let expected = json!({ "update": {
    "version": "0.4.1", "channel": "stable", "notes": "## Fixed\n- Crash on start", "critical": true,
    "published_at": 1_792_147_957_042_i64,
    "files": [{
      "name": NAME, "platform": "windows", "arch": "x86_64",
      "url": format!("https://api.orchester.cloud/v1/releases/0.4.1/files/{NAME}"),
      "size": 48_213_504, "sha256": DIGEST, "key_id": KEY_ID, "signature": SIGNATURE,
    }],
  }});
  round_trip(&answer, &expected);
}

#[test]
fn releases_upload_shapes_round_trip() {
  let release = NewRelease {
    version: version("0.5.0-beta.1"),
    notes: "Beta".to_owned(),
    critical: false,
    ci_run: None,
  };
  round_trip(
    &release,
    &json!({ "version": "0.5.0-beta.1", "notes": "Beta", "critical": false }),
  );
  let file = DeclaredFile {
    name: file_name(NAME),
    platform: ReleasePlatform::Windows,
    arch: ReleaseArch::X86_64,
    size: 48_213_504,
    sha256: digest(),
    key_id: key_id(),
    signature: signature(),
  };
  let expected = json!({
    "name": NAME, "platform": "windows", "arch": "x86_64", "size": 48_213_504,
    "sha256": DIGEST, "key_id": KEY_ID, "signature": SIGNATURE,
  });
  round_trip(&file, &expected);
  assert_eq!(file.signed(&version("0.5.0-beta.1")).version, version("0.5.0-beta.1"));
}

#[test]
fn releases_upload_names_the_parts_still_to_send() {
  let upload = FileUpload {
    name: file_name(NAME),
    size: 20_971_521,
    part_size: 8_388_608,
    parts: 3,
    received: vec![0, 2],
  };
  let expected = json!({ "name": NAME, "size": 20_971_521, "part_size": 8_388_608, "parts": 3, "received": [0, 2] });
  round_trip(&upload, &expected);
  assert_eq!(upload.missing(), [1]);
}

#[test]
fn releases_verification_says_each_files_verdict() {
  let verification = Verification {
    version: version("0.4.1"),
    files: vec![
      FileVerdict {
        name: file_name(NAME),
        result: FileResult::Accepted,
      },
      FileVerdict {
        name: file_name("Orchester-0.4.1-macos-aarch64.dmg"),
        result: FileResult::Rejected {
          reason: Rejection::Sha256Mismatch,
        },
      },
    ],
  };
  let expected = json!({ "version": "0.4.1", "files": [
    { "name": NAME, "result": "accepted" },
    { "name": "Orchester-0.4.1-macos-aarch64.dmg", "result": "rejected", "reason": "sha256_mismatch" },
  ]});
  round_trip(&verification, &expected);
  assert!(!verification.accepted());
  let reason = json!({ "name": NAME, "result": "rejected", "reason": "signature_mismatch" });
  let verdict: FileVerdict = serde_json::from_value(reason).expect("read");
  assert_eq!(
    verdict.result,
    FileResult::Rejected {
      reason: Rejection::SignatureMismatch
    }
  );
}
