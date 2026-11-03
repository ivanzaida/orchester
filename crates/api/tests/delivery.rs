//! Delivery reads (S7): a delivery's exact wire shape, its states, and unknown or malformed fields refused.

use orchester_api::v2::{BundleRef, DeliveryRead, DeliveryState};
use orchester_core::{
  blob::ContentHash,
  id::{ClaimId, OperationId, WorkId},
  run::CommitId,
  sharing::TaskRevision,
};
use serde_json::{Value, json};

const SOURCE: &str = "1111111111111111111111111111111111111111";
const BASE: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn delivery(authorization: Option<OperationId>) -> DeliveryRead {
  DeliveryRead {
    work_id: WorkId::generate(),
    claim_id: ClaimId::generate(),
    authorization_operation_id: authorization,
    source_commit: CommitId::new(SOURCE).expect("commit"),
    base_commit: CommitId::new(BASE).expect("commit"),
    bundle: BundleRef {
      sha256: ContentHash::new(&"a".repeat(64)).expect("hash"),
      schema_version: 1,
    },
    state: DeliveryState::Requested,
    revision: TaskRevision::try_from(3).expect("revision"),
  }
}

fn wire(read: &DeliveryRead) -> Value {
  json!({
    "work_id": read.work_id,
    "claim_id": read.claim_id,
    "authorization_operation_id": read.authorization_operation_id,
    "source_commit": SOURCE,
    "base_commit": BASE,
    "bundle": {"sha256": "a".repeat(64), "schema_version": 1},
    "state": "requested",
    "revision": 3
  })
}

#[test]
fn delivery_wire_shape_is_exact() {
  for authorization in [Some(OperationId::generate()), None] {
    let read = delivery(authorization);
    let wire = wire(&read);
    assert_eq!(serde_json::to_value(&read).expect("serializes"), wire);
    assert_eq!(serde_json::from_value::<DeliveryRead>(wire).expect("reads"), read);
  }
  let field_order = serde_json::to_string(&delivery(None)).expect("serializes");
  let places: Vec<usize> = [
    "work_id",
    "claim_id",
    "authorization_operation_id",
    "source_commit",
    "base_commit",
    "bundle",
    "state",
    "revision",
  ]
  .iter()
  .map(|field| field_order.find(&format!("\"{field}\":")).expect("sent"))
  .collect();
  assert!(places.windows(2).all(|pair| pair[0] < pair[1]), "{field_order}");
  assert!(
    field_order.contains("\"authorization_operation_id\":null"),
    "{field_order}"
  );
}

#[test]
fn delivery_state_wire_names() {
  for (state, name) in [
    (DeliveryState::Authorized, "authorized"),
    (DeliveryState::Requested, "requested"),
    (DeliveryState::Delivered, "delivered"),
    (DeliveryState::Failed, "failed"),
  ] {
    assert_eq!(serde_json::to_value(state).expect("serializes"), json!(name));
    assert_eq!(
      serde_json::from_value::<DeliveryState>(json!(name)).expect("reads"),
      state
    );
  }
  for unknown in ["merged", "Delivered", "pending"] {
    assert!(
      serde_json::from_value::<DeliveryState>(json!(unknown)).is_err(),
      "{unknown}"
    );
  }
}

#[test]
fn delivery_wire_shape_refuses_unknown_missing_and_malformed_fields() {
  let wire = wire(&delivery(None));
  let mut unknown = wire.clone();
  unknown["branch"] = json!("module/x");
  let refused = serde_json::from_value::<DeliveryRead>(unknown).expect_err("refused");
  assert!(refused.to_string().contains("unknown field"), "{refused}");
  let mut nested = wire.clone();
  nested["bundle"]["url"] = json!("x");
  assert!(serde_json::from_value::<DeliveryRead>(nested).is_err());

  let mut no_authorization = wire.clone();
  no_authorization
    .as_object_mut()
    .expect("an object")
    .remove("authorization_operation_id");
  assert!(serde_json::from_value::<DeliveryRead>(no_authorization).is_err());

  for (field, value) in [
    ("source_commit", json!(&SOURCE[..12])),
    ("base_commit", json!(BASE.to_uppercase())),
    ("state", json!("merged")),
    ("revision", json!(0)),
  ] {
    let mut malformed = wire.clone();
    malformed[field] = value;
    assert!(serde_json::from_value::<DeliveryRead>(malformed).is_err(), "{field}");
  }
}
