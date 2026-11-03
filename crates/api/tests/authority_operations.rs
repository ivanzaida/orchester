use orchester_api::v2::{
  AuthorityError, AuthorityErrorCode, AuthorityErrorShapeError, BundleRef, CommandRequest, CommandResult, DecisionId,
  ErrorDetails, EventCursor, ExpectedRevision, ExpectedRevisions, LocalEffect, OperationRead, RevisionTarget, Subject,
};
use orchester_core::{
  blob::ContentHash,
  id::{OperationId, TaskId, WorkId},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A command shaped as the contract's commands are; the real ones come with the stages that implement them.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Command {
  #[serde(rename = "claim.acquire")]
  Acquire { subject: Subject },
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum CommandValue {
  #[serde(rename = "claim.acquire")]
  Acquired { effects: Vec<LocalEffect> },
}

fn cursor() -> EventCursor {
  EventCursor::new("c1".to_owned()).expect("cursor")
}

fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(value: &T, wire: &Value) {
  assert_eq!(&serde_json::to_value(value).expect("serialize"), wire);
  assert_eq!(&serde_json::from_value::<T>(wire.clone()).expect("read"), value);
}

#[test]
fn authority_operations_subject_and_local_effect_shapes() {
  let task = TaskId::generate();
  roundtrip(&Subject::Task { id: task }, &json!({"kind":"task","id":task}));
  let work_id = WorkId::generate();
  let sha256 = ContentHash::new(&"a".repeat(64)).expect("hash");
  let bundle = BundleRef {
    sha256: sha256.clone(),
    schema_version: 2,
  };
  for (effect, wire) in [
    (
      LocalEffect::Start { work_id, bundle },
      json!({"kind":"start","work_id":work_id,"bundle":{"sha256":sha256,"schema_version":2}}),
    ),
    (
      LocalEffect::Pause { work_id },
      json!({"kind":"pause","work_id":work_id}),
    ),
    (LocalEffect::Stop { work_id }, json!({"kind":"stop","work_id":work_id})),
    (
      LocalEffect::Deliver { work_id },
      json!({"kind":"deliver","work_id":work_id}),
    ),
    (LocalEffect::None {}, json!({"kind":"none"})),
  ] {
    roundtrip(&effect, &wire);
  }
  for invalid in [
    json!({"kind":"none","work_id":work_id}),
    json!({"kind":"pause","work_id":work_id,"bundle":null}),
    json!({"kind":"start","work_id":work_id,"bundle":{"sha256":sha256,"schema_version":2,"url":"x"}}),
    json!({"kind":"resume","work_id":work_id}),
    json!({"kind":"deliver"}),
    json!({"kind":"deliver","work_id":work_id,"branch":"module/x"}),
  ] {
    assert!(serde_json::from_value::<LocalEffect>(invalid).is_err());
  }
  assert!(serde_json::from_value::<Subject>(json!({"kind":"task","id":task,"title":"x"})).is_err());
  assert!(serde_json::from_value::<Subject>(json!({"kind":"project","id":task})).is_err());
  assert!(serde_json::from_value::<EventCursor>(json!("")).is_err());
}

#[test]
fn authority_operations_command_request_and_result_shapes() {
  let operation_id = OperationId::generate();
  let task = TaskId::generate();
  let expected = ExpectedRevision::new(RevisionTarget::Task(task), 4).expect("revision");
  let request = CommandRequest {
    operation_id,
    expected: ExpectedRevisions::new(vec![expected]).expect("set"),
    command: Command::Acquire {
      subject: Subject::Task { id: task },
    },
  };
  let wire = json!({
    "operation_id":operation_id,
    "expected":[{"kind":"task","id":task,"revision":4}],
    "command":{"kind":"claim.acquire","subject":{"kind":"task","id":task}}
  });
  roundtrip(&request, &wire);
  let mut unknown = wire.clone();
  unknown["account"] = json!("someone");
  assert!(serde_json::from_value::<CommandRequest<Command>>(unknown).is_err());
  let mut duplicate = wire.clone();
  duplicate["expected"] = json!([{"kind":"task","id":task,"revision":4},{"kind":"task","id":task,"revision":5}]);
  assert!(serde_json::from_value::<CommandRequest<Command>>(duplicate).is_err());
  let mut zero = wire;
  zero["expected"][0]["revision"] = json!(0);
  assert!(serde_json::from_value::<CommandRequest<Command>>(zero).is_err());

  let result = CommandResult::committed(operation_id, CommandValue::Acquired { effects: Vec::new() }, cursor());
  let wire = json!({
    "operation_id":operation_id, "state":"committed",
    "result":{"kind":"claim.acquire","effects":[]}, "cursor":"c1"
  });
  roundtrip(&result, &wire);
  for (field, value) in [("state", json!("rejected")), ("receipt", json!(1))] {
    let mut invalid = wire.clone();
    invalid[field] = value;
    assert!(serde_json::from_value::<CommandResult<CommandValue>>(invalid).is_err());
  }
  let mut stateless = wire;
  stateless.as_object_mut().expect("object").remove("state");
  assert!(serde_json::from_value::<CommandResult<CommandValue>>(stateless).is_err());
}

#[test]
fn authority_operations_operation_read_roundtrips() {
  let operation_id = OperationId::generate();
  let response = CommandResult::committed(operation_id, CommandValue::Acquired { effects: Vec::new() }, cursor());
  let committed = OperationRead::Committed { operation_id, response };
  let wire = json!({
    "operation_id":operation_id, "state":"committed",
    "response":{"operation_id":operation_id,"state":"committed","result":{"kind":"claim.acquire","effects":[]},
      "cursor":"c1"}
  });
  roundtrip(&committed, &wire);
  assert_eq!(committed.operation_id(), operation_id);

  let error = AuthorityError::new(
    Some(operation_id),
    AuthorityErrorCode::DecisionRequired,
    ErrorDetails::DecisionRequired {
      decisions: vec![DecisionId::D1, DecisionId::D9],
    },
  )
  .expect("error");
  let rejected = OperationRead::<CommandValue>::Rejected { operation_id, error };
  let wire = json!({
    "operation_id":operation_id, "state":"rejected",
    "error":{"operation_id":operation_id,"code":"decision_required","details":{"decisions":["D1","D9"]}}
  });
  roundtrip(&rejected, &wire);
  let mut unknown = wire;
  unknown["response"] = json!(null);
  assert!(serde_json::from_value::<OperationRead<CommandValue>>(unknown).is_err());
}

#[test]
fn authority_operations_error_codes_map_to_status() {
  use AuthorityErrorCode as Code;
  for (code, wire, status) in [
    (Code::Unauthenticated, "unauthenticated", 401),
    (Code::NotFound, "not_found", 404),
    (Code::Forbidden, "forbidden", 403),
    (Code::Invalid, "invalid", 422),
    (Code::RevisionConflict, "revision_conflict", 409),
    (Code::OperationMismatch, "operation_mismatch", 409),
    (Code::ClaimConflict, "claim_conflict", 409),
    (Code::ClaimLost, "claim_lost", 409),
    (Code::ModuleRequired, "module_required", 409),
    (Code::ModuleClaimConflict, "module_claim_conflict", 409),
    (Code::TransitionDenied, "transition_denied", 409),
    (Code::DecisionRequired, "decision_required", 409),
    (Code::BundleMissing, "bundle_missing", 409),
    (Code::DeliveryNotAuthorized, "delivery_not_authorized", 403),
    (Code::CursorExpired, "cursor_expired", 410),
    (Code::UnsupportedContract, "unsupported_contract", 426),
    (Code::RateLimited, "rate_limited", 429),
    (Code::Unavailable, "unavailable", 503),
  ] {
    assert_eq!(code.status(), status, "{wire}");
    roundtrip(&code, &json!(wire));
  }
}

#[test]
fn authority_operations_error_details_belong_to_their_code() {
  let error = AuthorityError::new(None, AuthorityErrorCode::Unauthenticated, ErrorDetails::None).expect("error");
  roundtrip(&error, &json!({"code":"unauthenticated","details":{}}));
  assert_eq!(error.operation_id(), None);
  let decisions = ErrorDetails::DecisionRequired {
    decisions: vec![DecisionId::D2],
  };
  assert_eq!(
    AuthorityError::new(None, AuthorityErrorCode::Invalid, decisions),
    Err(AuthorityErrorShapeError::DetailsMismatch(AuthorityErrorCode::Invalid))
  );
  assert_eq!(
    AuthorityError::new(None, AuthorityErrorCode::DecisionRequired, ErrorDetails::None),
    Err(AuthorityErrorShapeError::DetailsMismatch(
      AuthorityErrorCode::DecisionRequired
    ))
  );
  let empty = ErrorDetails::DecisionRequired { decisions: Vec::new() };
  assert_eq!(
    AuthorityError::new(None, AuthorityErrorCode::DecisionRequired, empty),
    Err(AuthorityErrorShapeError::NoDecisions)
  );
  for invalid in [
    json!({"code":"invalid","details":{"decisions":["D1"]}}),
    json!({"code":"decision_required","details":{}}),
    json!({"code":"decision_required","details":{"decisions":["D10"]}}),
    json!({"code":"invalid","details":{"request":{"password":"x"}}}),
    json!({"code":"invalid","details":{},"message":"x"}),
    json!({"code":"invalid"}),
    json!({"code":"teapot","details":{}}),
  ] {
    assert!(serde_json::from_value::<AuthorityError>(invalid).is_err());
  }
}
