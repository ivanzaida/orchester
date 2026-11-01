//! The public Stable release's query and answer, written and read back.

use orchester_api::releases::{
  ReleaseArch, ReleasePlatform, StableFile, StableRelease, StableReleaseAnswer, StableReleaseQuery,
};
use serde_json::json;

use super::{
  DIGEST, file_name,
  shapes::{NAME, digest, round_trip},
  version,
};

#[test]
fn releases_stable_query_names_platform_and_arch() {
  let query = StableReleaseQuery {
    platform: ReleasePlatform::Macos,
    arch: ReleaseArch::Aarch64,
  };
  let expected = [("platform", "macos"), ("arch", "aarch64")];
  assert_eq!(query.query(), expected.map(|(key, value)| (key, value.to_owned())));
  round_trip(&query, &json!({ "platform": "macos", "arch": "aarch64" }));
}

#[test]
fn releases_stable_query_refuses_an_unknown_field_or_platform() {
  let install =
    json!({ "platform": "windows", "arch": "x86_64", "install_id": "0192f4d5-7c3a-7b2e-9a41-5d6c7e8f9a0b" });
  assert!(serde_json::from_value::<StableReleaseQuery>(install).is_err());
  let linux = json!({ "platform": "linux", "arch": "x86_64" });
  assert!(serde_json::from_value::<StableReleaseQuery>(linux).is_err());
}

#[test]
fn releases_stable_answers_no_release_as_null() {
  round_trip(&StableReleaseAnswer { release: None }, &json!({ "release": null }));
}

#[test]
fn releases_stable_answers_the_release_with_its_files() {
  let answer = StableReleaseAnswer {
    release: Some(StableRelease {
      version: version("0.4.1"),
      published_at: 1_792_147_957_042,
      files: vec![StableFile {
        name: file_name(NAME),
        platform: ReleasePlatform::Windows,
        arch: ReleaseArch::X86_64,
        size: 48_213_504,
        sha256: digest(),
      }],
    }),
  };
  let expected = json!({ "release": {
    "version": "0.4.1", "published_at": 1_792_147_957_042_i64,
    "files": [{ "name": NAME, "platform": "windows", "arch": "x86_64", "size": 48_213_504, "sha256": DIGEST }],
  }});
  round_trip(&answer, &expected);
}
