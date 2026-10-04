use orchester_api::SharingError;
use orchester_core::id::{ProjectId, TaskId, UserId};
use serde_json::{Value, json};

#[test]
fn s1b_every_sharing_error_detail_round_trips_on_the_client() {
  let user = json!({"id":UserId::generate(),"display_name":"Owner"});
  let task = json!({"id":TaskId::generate(),"number":1,"key":"ORC-1","former_key":null,
    "revision":1,"title":"Task","status":"draft","has_run":false,"created":{"by":user,"at":1},
    "updated":{"by":user,"at":1},"claim":null,"run":null});
  let cases = [
    (
      "remote_taken",
      json!({"project":ProjectId::generate(),"member":false,"owners":[user]}),
    ),
    ("not_member", json!({"owners":[user]})),
    (
      "revision_conflict",
      json!({"current":serde_json::from_value::<orchester_api::SharedTask>(task).expect("task")}),
    ),
    ("not_editable", json!({"reason":"queued_without_run"})),
    ("not_queueable", json!({"missing":["brief","criteria"]})),
    ("not_queueable", json!({"missing":[],"reason":"not_draft"})),
    ("organization", json!({"reason":"tag_not_found"})),
  ];
  for (code, details) in cases {
    round_trip(code, details);
  }
  let claim = json!({"id":orchester_core::id::ClaimId::generate(),"holder":user,"kind":"manual",
    "since":1,"heartbeat_at":1,"stale":false,"taken_over_from":null});
  for code in ["claim_held", "not_holder", "claim_not_stale", "claim_changed"] {
    round_trip(code, json!({"claim":claim}));
    round_trip(code, json!({"claim":null}));
  }
}

fn round_trip(code: &str, mut value: Value) {
  value["error"] = json!(code);
  value["message"] = json!("Diagnostic");
  let decoded: SharingError = serde_json::from_value(value.clone()).expect("client error details");
  assert_eq!(serde_json::to_value(decoded).expect("round trip"), value);
}

#[test]
fn s1b_claim_detail_requires_its_field_and_rejects_unrelated_fields() {
  for value in [json!({}), json!({"claim":null,"reason":"extra"})] {
    assert!(serde_json::from_value::<orchester_api::SharingErrorDetails>(value).is_err());
  }
  assert!(matches!(
    serde_json::from_value::<orchester_api::SharingErrorDetails>(json!({"missing":[]})).expect("missing details"),
    orchester_api::SharingErrorDetails::Missing { .. }
  ));
}
