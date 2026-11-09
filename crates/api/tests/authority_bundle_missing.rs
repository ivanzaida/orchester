//! `bundle_missing` says why when the server does: `details` is `{}` or `{reason:{kind,resource_id?,version?}}`. A
//! reason of a kind this version does not know reads as `other`, and a reason that grew a field is still read.

use orchester_api::v2::{
  AuthorityError, AuthorityErrorCode, AuthorityErrorShapeError, BundleMissingKind, BundleMissingReason, DecisionId,
  ErrorDetails,
};
use orchester_core::{id::ResourceId, resource::VersionNumber};
use serde_json::{Value, json};

fn bundle_missing(details: &Value) -> serde_json::Result<AuthorityError> {
  serde_json::from_value(json!({"code": "bundle_missing", "details": details}))
}

fn reason_of(error: &AuthorityError) -> &BundleMissingReason {
  let ErrorDetails::BundleMissing { reason } = error.details() else {
    panic!("no reason: {:?}", error.details());
  };
  reason
}

#[test]
fn bundle_missing_without_a_reason_reads_as_before() {
  let error = bundle_missing(&json!({})).expect("a refusal");
  assert_eq!(error.code(), AuthorityErrorCode::BundleMissing);
  assert_eq!(error.code().status(), 409);
  assert_eq!(error.details(), &ErrorDetails::None);
  assert_eq!(
    serde_json::to_value(&error).expect("wire"),
    json!({"code": "bundle_missing", "details": {}})
  );
}

#[test]
fn bundle_missing_with_a_reason_round_trips_for_every_kind() {
  let (profile, version) = (ResourceId::generate(), VersionNumber::FIRST);
  for kind in [
    "pipeline_missing",
    "file_missing",
    "profile_unreadable",
    "mcp_server_archived",
    "body_unreadable",
    "bundle_too_large",
    "other",
  ] {
    let wire = json!({"code": "bundle_missing", "details": {"reason": {"kind": kind, "resource_id": profile, "version": version}}});
    let error: AuthorityError = serde_json::from_value(wire.clone()).expect("a refusal");
    let reason = reason_of(&error);
    assert_eq!(reason.resource_id, Some(profile));
    assert_eq!(reason.version, Some(version));
    assert_eq!(serde_json::to_value(&error).expect("wire"), wire, "{kind}");
  }
}

#[test]
fn bundle_missing_reason_resource_and_version_are_optional() {
  let error = bundle_missing(&json!({"reason": {"kind": "bundle_too_large"}})).expect("a refusal");
  assert_eq!(
    reason_of(&error),
    &BundleMissingReason {
      kind: BundleMissingKind::BundleTooLarge,
      resource_id: None,
      version: None,
    }
  );
  assert_eq!(
    serde_json::to_value(&error).expect("wire"),
    json!({"code": "bundle_missing", "details": {"reason": {"kind": "bundle_too_large"}}})
  );
}

#[test]
fn bundle_missing_unknown_kind_reads_as_other_and_unknown_reason_fields_are_ignored() {
  let archived = ResourceId::generate();
  let error = bundle_missing(&json!({"reason": {"kind": "quota_exceeded", "resource_id": archived,
    "limit": 12, "detail": {"nested": true}}}))
  .expect("a refusal");
  assert_eq!(
    reason_of(&error),
    &BundleMissingReason {
      kind: BundleMissingKind::Other,
      resource_id: Some(archived),
      version: None,
    }
  );
}

#[test]
fn bundle_missing_reason_without_a_known_kind_reads_as_other() {
  let resource = ResourceId::generate();
  for reason in [
    json!({}),
    json!({"kind": null}),
    json!({"kind": 7}),
    json!({"kind": ["file_missing"]}),
    json!({"kind": {"name": "file_missing"}}),
    json!({"kind": "cli_kind_missing"}),
    json!({"kind": "mcp_servers_unresolved"}),
  ] {
    let error = bundle_missing(&json!({"reason": reason})).expect("a refusal");
    assert_eq!(reason_of(&error).kind, BundleMissingKind::Other, "{reason}");
  }
  let error = bundle_missing(&json!({"reason": {"kind": 7, "resource_id": resource}})).expect("a refusal");
  assert_eq!(reason_of(&error).resource_id, Some(resource));
}

#[test]
fn bundle_missing_details_stay_closed_around_the_reason() {
  for invalid in [
    json!({"reason": {"kind": "other", "resource_id": "not-an-id"}}),
    json!({"reason": {"kind": "other", "version": 0}}),
    json!({"reason": {"kind": "other"}, "path": "/srv/bundles/x"}),
    json!({"reason": {"kind": "other"}, "decisions": ["D1"]}),
  ] {
    assert!(bundle_missing(&invalid).is_err(), "{invalid}");
  }
  let reason = json!({"reason": {"kind": "profile_unreadable"}});
  for code in ["invalid", "decision_required", "claim_lost"] {
    let wire = json!({"code": code, "details": reason});
    assert!(serde_json::from_value::<AuthorityError>(wire).is_err(), "{code}");
  }
}

#[test]
fn bundle_missing_reason_belongs_to_bundle_missing_only() {
  let reason = BundleMissingReason {
    kind: BundleMissingKind::McpServerArchived,
    resource_id: None,
    version: None,
  };
  assert!(
    AuthorityError::new(
      None,
      AuthorityErrorCode::BundleMissing,
      ErrorDetails::BundleMissing { reason: reason.clone() }
    )
    .is_ok()
  );
  assert!(AuthorityError::new(None, AuthorityErrorCode::BundleMissing, ErrorDetails::None).is_ok());
  assert_eq!(
    AuthorityError::new(
      None,
      AuthorityErrorCode::Invalid,
      ErrorDetails::BundleMissing { reason }
    ),
    Err(AuthorityErrorShapeError::DetailsMismatch(AuthorityErrorCode::Invalid))
  );
  assert_eq!(
    AuthorityError::new(
      None,
      AuthorityErrorCode::BundleMissing,
      ErrorDetails::DecisionRequired {
        decisions: vec![DecisionId::D1]
      }
    ),
    Err(AuthorityErrorShapeError::DetailsMismatch(
      AuthorityErrorCode::BundleMissing
    ))
  );
}
