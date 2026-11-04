//! Module delivery (S7c): a module branch delivered to base without a work, its local effect, and both kept apart from
//! the task form of a delivery.

use orchester_api::v2::{BundleRef, DeliveryRead, DeliveryState, LocalEffect, ModuleDeliveryRead};
use orchester_core::{
  blob::ContentHash,
  id::{ClaimId, ModuleId, OperationId, TaskId, WorkId},
  run::{BranchName, CommitId},
  sharing::TaskRevision,
};
use serde_json::{Value, json};

const SOURCE: &str = "1111111111111111111111111111111111111111";
const BASE: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const BRANCH: &str = "module/github-sign-in";

fn delivery(authorization: Option<OperationId>) -> ModuleDeliveryRead {
  ModuleDeliveryRead {
    module_id: ModuleId::generate(),
    claim_id: ClaimId::generate(),
    authorization_operation_id: authorization,
    branch: BranchName::new(BRANCH).expect("branch"),
    source_commit: CommitId::new(SOURCE).expect("commit"),
    base_commit: CommitId::new(BASE).expect("commit"),
    state: DeliveryState::Authorized,
    revision: TaskRevision::try_from(2).expect("revision"),
  }
}

fn wire(read: &ModuleDeliveryRead) -> Value {
  json!({
    "subject": {"kind": "module", "id": read.module_id},
    "claim_id": read.claim_id,
    "authorization_operation_id": read.authorization_operation_id,
    "branch": BRANCH,
    "source_commit": SOURCE,
    "base_commit": BASE,
    "state": "authorized",
    "revision": 2
  })
}

#[test]
fn module_delivery_wire_shape_is_exact() {
  for authorization in [Some(OperationId::generate()), None] {
    let read = delivery(authorization);
    let wire = wire(&read);
    assert_eq!(serde_json::to_value(&read).expect("serializes"), wire);
    assert_eq!(serde_json::from_value::<ModuleDeliveryRead>(wire).expect("reads"), read);
  }
  let field_order = serde_json::to_string(&delivery(None)).expect("serializes");
  let places: Vec<usize> = [
    "subject",
    "claim_id",
    "authorization_operation_id",
    "branch",
    "source_commit",
    "base_commit",
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
fn module_delivery_refuses_unknown_missing_and_malformed_fields() {
  let wire = wire(&delivery(None));
  for (field, value) in [
    ("work_id", json!(WorkId::generate())),
    ("bundle", json!({"sha256": "a".repeat(64), "schema_version": 1})),
    ("path", json!("C:/repo")),
  ] {
    let mut unknown = wire.clone();
    unknown[field] = value;
    let refused = serde_json::from_value::<ModuleDeliveryRead>(unknown).expect_err("refused");
    assert!(refused.to_string().contains("unknown field"), "{field}: {refused}");
  }
  let mut nested = wire.clone();
  nested["subject"]["title"] = json!("x");
  assert!(serde_json::from_value::<ModuleDeliveryRead>(nested).is_err());

  for field in ["authorization_operation_id", "subject", "branch"] {
    let mut missing = wire.clone();
    missing.as_object_mut().expect("an object").remove(field);
    assert!(
      serde_json::from_value::<ModuleDeliveryRead>(missing).is_err(),
      "{field}"
    );
  }

  for (field, value) in [
    ("subject", json!({"kind": "task", "id": TaskId::generate()})),
    ("branch", json!("module x")),
    ("source_commit", json!(&SOURCE[..12])),
    ("base_commit", json!(BASE.to_uppercase())),
    ("state", json!("merged")),
    ("revision", json!(0)),
  ] {
    let mut malformed = wire.clone();
    malformed[field] = value;
    assert!(
      serde_json::from_value::<ModuleDeliveryRead>(malformed).is_err(),
      "{field}"
    );
  }
}

#[test]
fn module_delivery_and_task_delivery_forms_stay_apart() {
  let module = serde_json::to_value(delivery(None)).expect("serializes");
  assert!(serde_json::from_value::<DeliveryRead>(module).is_err());

  let task = DeliveryRead {
    work_id: WorkId::generate(),
    claim_id: ClaimId::generate(),
    authorization_operation_id: None,
    source_commit: CommitId::new(SOURCE).expect("commit"),
    base_commit: CommitId::new(BASE).expect("commit"),
    bundle: BundleRef {
      sha256: ContentHash::new(&"a".repeat(64)).expect("hash"),
      schema_version: 1,
    },
    state: DeliveryState::Requested,
    revision: TaskRevision::try_from(3).expect("revision"),
  };
  let task_wire = serde_json::to_value(&task).expect("serializes");
  assert!(task_wire.get("kind").is_none(), "{task_wire}");
  assert_eq!(
    serde_json::from_value::<DeliveryRead>(task_wire.clone()).expect("reads"),
    task
  );
  assert!(serde_json::from_value::<ModuleDeliveryRead>(task_wire).is_err());
}

#[test]
fn module_delivery_local_effect_shape() {
  let module_id = ModuleId::generate();
  let effect = LocalEffect::DeliverModule {
    module_id,
    branch: BranchName::new(BRANCH).expect("branch"),
    source_commit: CommitId::new(SOURCE).expect("commit"),
    base_commit: CommitId::new(BASE).expect("commit"),
  };
  let wire = json!({
    "kind": "deliver_module",
    "module_id": module_id,
    "branch": BRANCH,
    "source_commit": SOURCE,
    "base_commit": BASE
  });
  assert_eq!(serde_json::to_value(&effect).expect("serializes"), wire);
  assert_eq!(
    serde_json::from_value::<LocalEffect>(wire.clone()).expect("reads"),
    effect
  );

  let mut with_path = wire.clone();
  with_path["path"] = json!("C:/repo");
  assert!(serde_json::from_value::<LocalEffect>(with_path).is_err());
  let mut with_work = wire.clone();
  with_work["work_id"] = json!(WorkId::generate());
  assert!(serde_json::from_value::<LocalEffect>(with_work).is_err());
  for field in ["module_id", "branch", "source_commit", "base_commit"] {
    let mut missing = wire.clone();
    missing.as_object_mut().expect("an object").remove(field);
    assert!(serde_json::from_value::<LocalEffect>(missing).is_err(), "{field}");
  }
  let mut short = wire;
  short["source_commit"] = json!(&SOURCE[..7]);
  assert!(serde_json::from_value::<LocalEffect>(short).is_err());
}
