//! The run progress report's body (`PUT …/tasks/{task}/run`, ORC-284): the contract's field names and phase words,
//! the fields the server sets itself refused, and its answer, the task.
//!
//! `cargo test -p orchester-api --test run_report claim_lifecycle_`

use orchester_api::{RunReport, TaskResponse};
use orchester_core::run::TaskPhase;
use serde_json::json;

fn merged() -> RunReport {
  RunReport {
    number: 3,
    phase: TaskPhase::Merged,
    step: None,
    started_at: 1_000,
    phase_since: 2_000,
  }
}

#[test]
fn claim_lifecycle_a_report_is_the_run_status_without_its_holder_and_update_time() {
  assert_eq!(
    serde_json::to_value(merged()).expect("serialize"),
    json!({ "number": 3, "phase": "merged", "step": null, "started_at": 1_000, "phase_since": 2_000 })
  );
  let running = json!({ "number": 1, "phase": "stopped_at_limit", "step": "Implement", "started_at": 5,
    "phase_since": 6 });
  let read: RunReport = serde_json::from_value(running).expect("deserialize");
  assert_eq!(read.phase, TaskPhase::StoppedAtLimit);
  assert_eq!(read.step.as_deref(), Some("Implement"));
  let without_step: RunReport = serde_json::from_value(json!({ "number": 3, "phase": "merged", "started_at": 1_000,
    "phase_since": 2_000 }))
  .expect("a report without a step");
  assert_eq!(without_step, merged());
}

#[test]
fn claim_lifecycle_a_report_names_neither_holder_nor_update_time() {
  for (field, value) in [
    ("holder", json!({ "id": "u", "display_name": "Alice" })),
    ("updated_at", json!(3_000)),
  ] {
    let mut body = serde_json::to_value(merged()).expect("serialize");
    body[field] = value;
    assert!(
      serde_json::from_value::<RunReport>(body).is_err(),
      "{field} is the server's to set"
    );
  }
  let unknown = json!({ "number": 3, "phase": "done", "step": null, "started_at": 1, "phase_since": 2 });
  assert!(
    serde_json::from_value::<RunReport>(unknown).is_err(),
    "only core's phases"
  );
}

#[test]
fn claim_lifecycle_the_answer_is_the_task_with_its_claim_ended() {
  let answer = json!({ "task": {
    "id": "01a183cf-b73c-71db-8a62-bd509770cd4d", "number": 108, "key": "ORC-108", "former_key": null,
    "revision": 4, "title": "Status bar", "status": "queued", "has_run": true,
    "created": { "by": { "id": "01a183cf-b73c-71db-8a62-bd509770cd4e", "display_name": "Alice" }, "at": 1 },
    "updated": { "by": { "id": "01a183cf-b73c-71db-8a62-bd509770cd4e", "display_name": "Alice" }, "at": 2 },
    "claim": null,
    "run": { "holder": { "id": "01a183cf-b73c-71db-8a62-bd509770cd4e", "display_name": "Alice" }, "number": 3,
      "phase": "merged", "step": null, "started_at": 1_000, "phase_since": 2_000, "updated_at": 2_001 }
  }});
  let read: TaskResponse = serde_json::from_value(answer).expect("the task");
  assert!(read.task.claim.is_none());
  assert_eq!(read.task.run.map(|run| run.phase), Some(TaskPhase::Merged));
}
