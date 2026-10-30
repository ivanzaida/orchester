//! The JSON of each release shape, written and read back.

use std::fmt::Debug;

use orchester_api::releases::{
  AdminFile, AdoptionDay, Channel, DeclaredFile, FileDigest, FileResult, FileSignature, FileUpload, FileVerdict,
  NewRelease, OfferedFile, OfferedRelease, PercentChange, Rejection, ReleaseAction, ReleaseActivity, ReleaseArch,
  ReleaseChanges, ReleaseDetails, ReleaseKeyId, ReleasePlatform, ReleaseStage, ReleaseSummary, RolloutPercent,
  UpdateCheck, UpdateCheckAnswer, Verification,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use super::{DIGEST, KEY_ID, SIGNATURE, file_name, version};

const NAME: &str = "Orchester-0.4.1-windows-x86_64.msi";

/// `value` is written as `expected` and read back as itself.
fn round_trip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T, expected: &Value) {
  assert_eq!(&serde_json::to_value(value).expect("written"), expected);
  assert_eq!(&serde_json::from_value::<T>(expected.clone()).expect("read"), value);
}

fn digest() -> FileDigest {
  FileDigest::parse(DIGEST).expect("a digest")
}

fn key_id() -> ReleaseKeyId {
  ReleaseKeyId::parse(KEY_ID).expect("a key id")
}

fn signature() -> FileSignature {
  FileSignature::parse(SIGNATURE).expect("a signature")
}

fn percent(value: u8) -> RolloutPercent {
  RolloutPercent::new(value).expect("a percentage")
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

fn summary(stage: ReleaseStage) -> ReleaseSummary {
  ReleaseSummary {
    version: version("0.4.1"),
    stage,
    critical: false,
    created_at: 1_792_000_000_000,
    published_at: Some(1_792_100_000_000),
    installs: 1_204,
    files_deleted: false,
  }
}

#[test]
fn releases_stages_carry_their_rollout() {
  let stable = summary(ReleaseStage::Stable {
    percent: percent(25),
    paused: true,
  });
  let expected = json!({
    "version": "0.4.1", "stage": "stable", "percent": 25, "paused": true, "critical": false,
    "created_at": 1_792_000_000_000_i64, "published_at": 1_792_100_000_000_i64, "installs": 1_204,
    "files_deleted": false,
  });
  round_trip(&stable, &expected);
  assert_eq!(stable.stage.channels(), [Channel::Stable, Channel::Beta]);
  let mut beta = expected;
  let fields = beta.as_object_mut().expect("an object");
  fields.remove("percent");
  fields.remove("paused");
  fields.insert("stage".to_owned(), json!("beta"));
  round_trip(&summary(ReleaseStage::Beta), &beta);
  assert!(ReleaseStage::Withdrawn.channels().is_empty());
  assert!(ReleaseStage::Draft.channels().is_empty());
}

#[test]
fn releases_details_carry_files_adoption_and_activity() {
  let details = ReleaseDetails {
    summary: summary(ReleaseStage::Beta),
    notes: "Notes".to_owned(),
    files: vec![AdminFile {
      name: file_name(NAME),
      platform: ReleasePlatform::Windows,
      arch: ReleaseArch::X86_64,
      size: 48_213_504,
      sha256: digest(),
      key_id: key_id(),
      downloads: 980,
    }],
    adoption: vec![AdoptionDay {
      day: "2026-10-29".to_owned(),
      installs: 311,
    }],
    activity: vec![
      ReleaseActivity {
        at: 1_792_100_000_000,
        actor: Some("Ivan".to_owned()),
        action: ReleaseAction::Promoted { percent: percent(10) },
      },
      ReleaseActivity {
        at: 1_792_000_000_000,
        actor: None,
        action: ReleaseAction::Created,
      },
    ],
  };
  let value = serde_json::to_value(&details).expect("written");
  assert_eq!(value["stage"], "beta");
  assert_eq!(value["files"][0]["downloads"], 980);
  assert_eq!(value["adoption"][0], json!({ "day": "2026-10-29", "installs": 311 }));
  assert_eq!(
    value["activity"][0],
    json!({ "at": 1_792_100_000_000_i64, "actor": "Ivan", "action": "promoted", "percent": 10 })
  );
  assert_eq!(
    value["activity"][1],
    json!({ "at": 1_792_000_000_000_i64, "actor": null, "action": "created" })
  );
  assert_eq!(serde_json::from_value::<ReleaseDetails>(value).expect("read"), details);
}

#[test]
fn releases_admin_requests_round_trip() {
  round_trip(&PercentChange { percent: percent(50) }, &json!({ "percent": 50 }));
  assert!(serde_json::from_value::<PercentChange>(json!({ "percent": 101 })).is_err());
  let changes = ReleaseChanges {
    notes: None,
    critical: Some(true),
  };
  round_trip(&changes, &json!({ "critical": true }));
  assert!(serde_json::from_value::<ReleaseChanges>(json!({ "channel": "beta" })).is_err());
}
