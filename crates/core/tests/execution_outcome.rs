//! What a run reports about authorized work (S9): the execution outcome's exact wire shapes, its closed failure
//! reasons, and the per-run sequence rule that orders reports.

use orchester_core::{
  resource::StepKey,
  run::{
    ExecutionFailure, ExecutionOutcome, ReportOrder, RunFailure, SequenceRegressed, WorktreeFailure, order_report,
  },
};
use serde_json::json;

#[test]
fn execution_outcome_wire_shapes_are_exact() {
  let failed = ExecutionOutcome::Failed {
    reason: ExecutionFailure::StepFailed,
  };
  for (outcome, wire) in [
    (ExecutionOutcome::Progress {}, json!({"kind":"progress"})),
    (ExecutionOutcome::Succeeded {}, json!({"kind":"succeeded"})),
    (failed, json!({"kind":"failed","reason":"step_failed"})),
    (ExecutionOutcome::Cancelled {}, json!({"kind":"cancelled"})),
  ] {
    assert_eq!(serde_json::to_value(outcome).expect("serializes"), wire);
    assert_eq!(
      serde_json::from_value::<ExecutionOutcome>(wire).expect("reads"),
      outcome
    );
  }
}

#[test]
fn execution_outcome_refuses_unknown_fields_and_kinds() {
  for invalid in [
    json!({"kind":"progress","percent":40}),
    json!({"kind":"succeeded","done":true}),
    json!({"kind":"cancelled","by":"x"}),
    json!({"kind":"failed","reason":"step_failed","message":"token=abc"}),
    json!({"kind":"failed"}),
    json!({"kind":"failed","reason":"agent crashed"}),
    json!({"kind":"completed"}),
    json!({"kind":"awaiting_human"}),
    json!({}),
  ] {
    assert!(
      serde_json::from_value::<ExecutionOutcome>(invalid.clone()).is_err(),
      "{invalid}"
    );
  }
}

#[test]
fn execution_outcome_failure_reasons_are_closed() {
  for (reason, wire) in [
    (ExecutionFailure::StepFailed, "step_failed"),
    (ExecutionFailure::RetriesExhausted, "retries_exhausted"),
    (ExecutionFailure::WorktreeNotCreated, "worktree_not_created"),
    (ExecutionFailure::LimitReached, "limit_reached"),
    (ExecutionFailure::RunnerFailed, "runner_failed"),
  ] {
    assert_eq!(serde_json::to_value(reason).expect("serializes"), json!(wire));
    assert_eq!(
      serde_json::from_value::<ExecutionFailure>(json!(wire)).expect("reads"),
      reason
    );
  }
  let step = StepKey::new("implement").expect("a step key");
  for (failure, reason) in [
    (RunFailure::StepFailed(step.clone()), ExecutionFailure::StepFailed),
    (RunFailure::RetriesExhausted(step), ExecutionFailure::RetriesExhausted),
    (
      RunFailure::WorktreeNotCreated(WorktreeFailure::UnknownBase),
      ExecutionFailure::WorktreeNotCreated,
    ),
  ] {
    assert_eq!(ExecutionFailure::from(&failure), reason);
  }
}

#[test]
fn execution_outcome_sequence_greater_is_accepted() {
  let progress = ExecutionOutcome::Progress {};
  assert_eq!(order_report(None, 0, &progress), Ok(ReportOrder::Next));
  assert_eq!(order_report(Some((3, &progress)), 4, &progress), Ok(ReportOrder::Next));
  assert_eq!(
    order_report(Some((3, &progress)), u64::MAX, &ExecutionOutcome::Succeeded {}),
    Ok(ReportOrder::Next)
  );
}

#[test]
fn execution_outcome_sequence_equal_is_duplicate_only_with_same_content() {
  let progress = ExecutionOutcome::Progress {};
  assert_eq!(
    order_report(Some((5, &progress)), 5, &progress),
    Ok(ReportOrder::Duplicate)
  );
  assert_eq!(
    order_report(Some((5, &progress)), 5, &ExecutionOutcome::Cancelled {}),
    Err(SequenceRegressed { last: 5, sequence: 5 })
  );
}

#[test]
fn execution_outcome_sequence_lower_is_refused() {
  let succeeded = ExecutionOutcome::Succeeded {};
  for sequence in [0, 4] {
    assert_eq!(
      order_report(Some((5, &succeeded)), sequence, &succeeded),
      Err(SequenceRegressed { last: 5, sequence })
    );
  }
}
