use orchester_api::{ApiError, ErrorCode, SharingError, SharingErrorDetails};
use orchester_core::id::ResourceId;
use serde_json::{Value, json};

use crate::fixtures::{hash, resource, version};

fn error(code: &str, mut details: Value) -> Value {
  details["error"] = json!(code);
  details["message"] = json!("Diagnostic");
  details
}

#[test]
fn resources_every_refusal_without_details_round_trips_not_admin_included() {
  for (code, expected) in [
    ("not_admin", ErrorCode::NotAdmin),
    ("archived", ErrorCode::Archived),
    ("hash_mismatch", ErrorCode::HashMismatch),
    ("payload_too_large", ErrorCode::PayloadTooLarge),
    ("id_taken", ErrorCode::IdTaken),
    ("invalid", ErrorCode::Invalid),
    ("not_found", ErrorCode::NotFound),
  ] {
    let value = error(code, json!({}));
    let decoded: ApiError = serde_json::from_value(value.clone()).expect("refusal");
    assert_eq!(decoded.error, expected);
    assert_eq!(serde_json::to_value(decoded).expect("round trip"), value);
  }
}

#[test]
fn resources_every_refusal_with_details_round_trips_as_its_detail() {
  let id = ResourceId::generate();
  let cases = [
    ("slug_taken", json!({"resource":resource(id, "pipeline")}), "resource"),
    ("version_conflict", json!({"latest":version(id, 4)}), "version"),
    ("missing_files", json!({"hashes":[hash('c'), hash('d')]}), "files"),
    (
      "revision_conflict",
      json!({"current":resource(id, "tool")}),
      "resource_revision",
    ),
    ("invalid", json!({"reason":"references"}), "reason"),
    ("invalid", json!({"reason":"skill_size"}), "reason"),
  ];
  for (code, details, expected) in cases {
    let value = error(code, details);
    let decoded: SharingError = serde_json::from_value(value.clone()).expect("refusal with details");
    let variant = match decoded.details {
      SharingErrorDetails::Resource { .. } => "resource",
      SharingErrorDetails::Version { .. } => "version",
      SharingErrorDetails::Files { .. } => "files",
      SharingErrorDetails::ResourceRevision { .. } => "resource_revision",
      SharingErrorDetails::Reason { .. } => "reason",
      ref other => panic!("{code} read as {other:?}"),
    };
    assert_eq!(variant, expected, "{code}");
    assert_eq!(serde_json::to_value(decoded).expect("round trip"), value);
  }
}

#[test]
fn resources_missing_files_names_only_content_hashes() {
  let details = json!({"hashes":["not-a-hash"]});
  assert!(!matches!(
    serde_json::from_value::<SharingErrorDetails>(details),
    Ok(SharingErrorDetails::Files { .. })
  ));
}
