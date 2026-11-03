//! Work reports (S9, `orchester_api::v2`): the exact wire shapes of `ArtifactRef`, `AuthorityRef`, `WorkReport` and the
//! `work.changed` event, unknown and missing fields refused, and the report's sequence rule.

use orchester_api::{
  UserRef,
  v2::{
    ArtifactKind, ArtifactRef, AuthorityRef, BundleRef, ExecutionFailure, ExecutionOutcome, ExecutionState, Subject,
    WorkChanged, WorkEvent, WorkRead, WorkReport,
  },
};
use orchester_core::{
  blob::ContentHash,
  id::{ClaimId, RunId, TaskId, UserId, WorkEventId, WorkId},
  run::{ReportOrder, SequenceRegressed, order_report},
  sharing::TaskRevision,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(value: &T, wire: &Value) {
  assert_eq!(&serde_json::to_value(value).expect("serialize"), wire);
  assert_eq!(&serde_json::from_value::<T>(wire.clone()).expect("read"), value);
}

/// `wire` without `field`, and with an extra one: both refused.
fn refuses_missing_and_unknown<T: for<'de> Deserialize<'de>>(wire: &Value, field: &str) {
  let mut missing = wire.clone();
  missing.as_object_mut().expect("an object").remove(field);
  assert!(serde_json::from_value::<T>(missing).is_err(), "{field} is required");
  let mut unknown = wire.clone();
  unknown
    .as_object_mut()
    .expect("an object")
    .insert("path".to_owned(), json!("C:/Users/ada/report.md"));
  assert!(
    serde_json::from_value::<T>(unknown).is_err(),
    "unknown fields are refused"
  );
}

fn hash(digit: &str) -> ContentHash {
  ContentHash::new(&digit.repeat(64)).expect("hash")
}

fn report(sequence: u64, outcome: ExecutionOutcome) -> (WorkReport, Value) {
  let (task, claim_id, work_id) = (TaskId::generate(), ClaimId::generate(), WorkId::generate());
  let (run_id, event_id) = (RunId::generate(), WorkEventId::generate());
  let report = WorkReport {
    authority: AuthorityRef {
      subject: Subject::Task { id: task },
      claim_id,
      work_id: Some(work_id),
    },
    run_id,
    event_id,
    sequence,
    outcome,
    artifacts: vec![ArtifactRef {
      sha256: hash("a"),
      kind: ArtifactKind::Report,
      size_bytes: 2_048,
    }],
  };
  let wire = json!({
    "authority":{"subject":{"kind":"task","id":task},"claim_id":claim_id,"work_id":work_id},
    "run_id":run_id,
    "event_id":event_id,
    "sequence":sequence,
    "outcome":serde_json::to_value(outcome).expect("outcome"),
    "artifacts":[{"sha256":"a".repeat(64),"kind":"report","size_bytes":2_048}]
  });
  (report, wire)
}

#[test]
fn work_report_artifact_ref_shape() {
  for (kind, name) in [
    (ArtifactKind::Report, "report"),
    (ArtifactKind::Patch, "patch"),
    (ArtifactKind::Log, "log"),
  ] {
    let artifact = ArtifactRef {
      sha256: hash("c"),
      kind,
      size_bytes: 7,
    };
    roundtrip(&artifact, &json!({"sha256":"c".repeat(64),"kind":name,"size_bytes":7}));
  }
  let wire = json!({"sha256":"c".repeat(64),"kind":"log","size_bytes":7});
  for field in ["sha256", "kind", "size_bytes"] {
    refuses_missing_and_unknown::<ArtifactRef>(&wire, field);
  }
  for (field, value) in [
    ("kind", json!("screenshot")),
    ("kind", json!("Report")),
    ("sha256", json!("C".repeat(64))),
    ("size_bytes", json!(-1)),
  ] {
    let mut malformed = wire.clone();
    malformed[field] = value;
    assert!(serde_json::from_value::<ArtifactRef>(malformed).is_err(), "{field}");
  }
}

#[test]
fn work_report_authority_ref_shape() {
  let (task, claim_id) = (TaskId::generate(), ClaimId::generate());
  let without_work = AuthorityRef {
    subject: Subject::Task { id: task },
    claim_id,
    work_id: None,
  };
  let wire = json!({"subject":{"kind":"task","id":task},"claim_id":claim_id,"work_id":null});
  roundtrip(&without_work, &wire);
  refuses_missing_and_unknown::<AuthorityRef>(&wire, "work_id");
  refuses_missing_and_unknown::<AuthorityRef>(&wire, "claim_id");
}

#[test]
fn work_report_wire_shape_is_exact() {
  let outcomes = [
    ExecutionOutcome::Progress {},
    ExecutionOutcome::Succeeded {},
    ExecutionOutcome::Failed {
      reason: ExecutionFailure::LimitReached,
    },
    ExecutionOutcome::Cancelled {},
  ];
  for outcome in outcomes {
    let (report, wire) = report(3, outcome);
    roundtrip(&report, &wire);
  }
  let (report, wire) = report(3, ExecutionOutcome::Progress {});
  let sent = serde_json::to_string(&report).expect("serializes");
  let places: Vec<usize> = ["authority", "run_id", "event_id", "sequence", "outcome", "artifacts"]
    .iter()
    .map(|field| sent.find(&format!("\"{field}\":")).expect("sent"))
    .collect();
  assert!(places.windows(2).all(|pair| pair[0] < pair[1]), "{sent}");
  assert!(sent.contains(r#""outcome":{"kind":"progress"}"#), "{sent}");
  for field in ["authority", "run_id", "event_id", "sequence", "outcome", "artifacts"] {
    refuses_missing_and_unknown::<WorkReport>(&wire, field);
  }
  for (pointer, value) in [
    (
      "/outcome",
      json!({"kind":"failed","reason":"step_failed","message":"x"}),
    ),
    ("/outcome", json!({"kind":"completed"})),
    ("/sequence", json!(-1)),
    ("/artifacts/0/path", json!("C:/x")),
    ("/authority/session", json!("s")),
  ] {
    let mut malformed = wire.clone();
    match malformed.pointer_mut(pointer) {
      Some(slot) => *slot = value,
      None => {
        let (parent, field) = pointer.rsplit_once('/').expect("a pointer");
        malformed
          .pointer_mut(parent)
          .and_then(Value::as_object_mut)
          .expect("an object")
          .insert(field.to_owned(), value);
      }
    }
    assert!(serde_json::from_value::<WorkReport>(malformed).is_err(), "{pointer}");
  }
}

#[test]
fn work_report_sequence_rule() {
  let (accepted, _) = report(4, ExecutionOutcome::Progress {});
  let mut next = accepted.clone();
  next.sequence = 5;
  next.event_id = WorkEventId::generate();
  assert_eq!(order_report(Some((4, &accepted)), 5, &next), Ok(ReportOrder::Next));
  assert_eq!(
    order_report(Some((4, &accepted)), 4, &accepted.clone()),
    Ok(ReportOrder::Duplicate)
  );
  let mut other = accepted.clone();
  other.event_id = WorkEventId::generate();
  assert_eq!(
    order_report(Some((4, &accepted)), 4, &other),
    Err(SequenceRegressed { last: 4, sequence: 4 })
  );
  let mut delayed = accepted.clone();
  delayed.sequence = 3;
  assert_eq!(
    order_report(Some((4, &accepted)), 3, &delayed),
    Err(SequenceRegressed { last: 4, sequence: 3 })
  );
}

fn work() -> (WorkRead, Value) {
  let (id, task, claim_id, run) = (
    WorkId::generate(),
    TaskId::generate(),
    ClaimId::generate(),
    RunId::generate(),
  );
  let work = WorkRead {
    id,
    subject: Subject::Task { id: task },
    claim_id,
    revision: TaskRevision::try_from(2).expect("revision"),
    state: ExecutionState::Succeeded,
    runs: vec![run],
    bundle: BundleRef {
      sha256: hash("b"),
      schema_version: 1,
    },
    created_at: 3_000,
    updated_at: 4_000,
  };
  let wire = json!({
    "id":id,"subject":{"kind":"task","id":task},"claim_id":claim_id,"revision":2,
    "state":"succeeded","runs":[run],"bundle":{"sha256":"b".repeat(64),"schema_version":1},
    "created_at":3_000,"updated_at":4_000
  });
  (work, wire)
}

#[test]
fn work_report_work_changed_shape() {
  let (work, work_wire) = work();
  let changed = WorkChanged { work: work.clone() };
  let wire = json!({"work":work_wire});
  roundtrip(&changed, &wire);
  refuses_missing_and_unknown::<WorkChanged>(&wire, "work");
  let mut nested = wire;
  nested["work"]["outcome"] = json!({"kind":"succeeded"});
  assert!(serde_json::from_value::<WorkChanged>(nested).is_err());

  let by = UserRef {
    id: UserId::generate(),
    display_name: "Ada".to_owned(),
  };
  let delta = json!({
    "kind":"work.changed",
    "data":{"id":9,"by":{"id":by.id,"display_name":"Ada"},"actor":"orchester","at":5_000,"work":work_wire}
  });
  let event: WorkEvent = serde_json::from_value(delta.clone()).expect("a work event");
  let WorkEvent::Changed(change) = &event;
  assert_eq!((change.id, &change.change.work), (9, &work));
  assert_eq!(serde_json::to_value(&event).expect("serializes"), delta);
  assert!(serde_json::from_value::<orchester_api::Event>(delta.clone()).is_err());
  let mut missing = delta;
  missing["data"].as_object_mut().expect("an object").remove("work");
  assert!(serde_json::from_value::<WorkEvent>(missing).is_err());
}
