//! Test stays explicit, renders durable results and stops the durable running submission.
use orchester_core::{
  connection::{DecisionCheckResult, DecisionConnection},
  id::IdempotencyKey,
  run::review::{
    AutoModeSettings, DecisionTestReport, DecisionTestStatus, Probability, ReviewerChoice, ThresholdFigures,
  },
  time::Timestamp,
};

use super::{
  decision::{connection, open},
  harness::Seeded,
};

fn report(connection: &DecisionConnection, status: DecisionTestStatus) -> DecisionTestReport {
  DecisionTestReport {
    id: IdempotencyKey::generate(),
    connection: connection.id(),
    base_url: connection.base_url().clone(),
    key: connection.key(),
    model: connection.models()[0].clone(),
    started_at: Timestamp::now(),
    status,
    requests: 100,
    tested: 90,
    previously_asked: Some(9),
    in_flight: 0,
    failed: 0,
    too_long: 0,
    median_ms: Some(182),
    input_tokens: 21000,
    unknown_usage: 0,
    omitted_fields: vec![],
    thresholds: [0.5, 0.8, 0.9, 0.95]
      .into_iter()
      .map(|threshold| ThresholdFigures {
        threshold: Probability::new(threshold).expect("threshold"),
        approves: 81,
        asks_person: 9,
        previously_asked_now_approved: 3,
      })
      .collect(),
  }
}

fn store(seeded: &Seeded, report: &DecisionTestReport) {
  assert!(seeded.db().begin_decision_test(report).expect("store report"));
}

#[test]
fn connections_decision_test_empty_table_never_submits_on_open() {
  let mut seeded = Seeded::new("connections");
  let connection = connection(&seeded, DecisionCheckResult::Connected);
  open(&mut seeded, connection.id());
  seeded.until("test read", |s| s.text("decision-test-meta").contains("No test yet"));
  assert!(seeded.text("decision-test-start").contains("Test"));
  assert!(seeded.text("decision-test-table").contains("Approves on its own"));
  assert!(!seeded.exists("decision-test-threshold-0.80"));
  assert!(
    seeded
      .db()
      .latest_decision_test(connection.id())
      .expect("read")
      .is_none()
  );
  seeded.click("decision-test-start");
  seeded.until("explicit test failure without runner", |s| {
    s.exists("decision-test-error")
  });
  assert!(
    seeded
      .db()
      .latest_decision_test(connection.id())
      .expect("read")
      .is_none()
  );
}

#[test]
fn connections_decision_test_last_results_mark_only_the_matching_reviewer_model() {
  let mut seeded = Seeded::new("connections");
  let connection = connection(&seeded, DecisionCheckResult::Connected);
  let report = report(&connection, DecisionTestStatus::Completed);
  store(&seeded, &report);
  seeded
    .db()
    .save_auto_mode_settings(&AutoModeSettings {
      reviewer: Some(ReviewerChoice::decision(connection.id(), report.model, None)),
    })
    .expect("reviewer");
  open(&mut seeded, connection.id());
  seeded.until("results", |s| s.exists("decision-test-threshold-0.80"));
  assert!(
    seeded
      .text("decision-test-threshold-0.80")
      .contains("used by the reviewer")
  );
  assert!(!seeded.text("decision-test-threshold-0.50").contains("used by"));
  assert!(seeded.text("decision-test-threshold-0.80").contains("90.0%"));
  for threshold in ["0.50", "0.80", "0.90", "0.95"] {
    assert!(
      seeded
        .text(&format!("decision-test-threshold-{threshold}"))
        .contains("3 of 9")
    );
  }
  for words in ["182 ms", "21000", "90 of 100", "jev-latest"] {
    assert!(seeded.text("decision-test-meta").contains(words), "{words}");
  }
  assert!(seeded.text("decision-test-start").contains("Test again"));
  seeded.list();
  seeded
    .db()
    .save_auto_mode_settings(&AutoModeSettings {
      reviewer: Some(ReviewerChoice::decision(
        connection.id(),
        orchester_core::resource::ModelId::new("another-model").expect("model"),
        None,
      )),
    })
    .expect("changed reviewer");
  open(&mut seeded, connection.id());
  seeded.until("old model results", |s| s.exists("decision-test-threshold-0.80"));
  assert!(!seeded.text("decision-test-table").contains("used by the reviewer"));
}

#[test]
fn connections_decision_test_stop_targets_the_running_report_and_keeps_partial_results() {
  let mut seeded = Seeded::new("connections");
  let connection = connection(&seeded, DecisionCheckResult::Connected);
  let mut report = report(&connection, DecisionTestStatus::Running);
  let mut legacy = serde_json::to_value(&report).expect("serialize legacy report");
  legacy
    .as_object_mut()
    .expect("report object")
    .remove("previously_asked");
  report = serde_json::from_value(legacy).expect("read report without denominator");
  assert_eq!(report.previously_asked, None);
  report.in_flight = 8;
  store(&seeded, &report);
  open(&mut seeded, connection.id());
  seeded.until("running", |s| s.exists("decision-test-stop"));
  assert!(seeded.text("decision-test-meta").contains("90 of 100"));
  seeded.click("decision-test-stop");
  seeded.until("stop persisted", |s| {
    s.db().decision_test_stop_requested(report.id).expect("flag")
  });
  report.status = DecisionTestStatus::Stopped;
  report.in_flight = 0;
  seeded
    .db()
    .save_decision_test(&report)
    .expect("runner finished stopping");
  seeded.until("stopped", |s| s.text("decision-test-meta").contains("Stopped"));
  assert!(seeded.text("decision-test-threshold-0.80").contains("3 of —"));
  assert!(seeded.exists("decision-test-start"));
}
