//! Flaky tests and quarantine (`orchester_core::test_step::flaky`, ORC-259): a test flaky in three runs within
//! fourteen days is watched; a quarantined test that fails again does not fail its suite, is shown to the steps after
//! a passed Test step, never to the writing step, and is kept in the flaky history beside the flaky ones.

use orchester_core::{
  id::RunId,
  resource::{Slug, TestLayer},
  test_report::{self, SuiteRun, SuiteRunEnd, TestRunId},
  test_step::{
    SuiteOutcome, TestStepResult,
    flaky::{
      FlakyKind, FlakyOutcome, Quarantine, Standing, TestKey, TestKeyError, WATCH_AFTER, WATCH_WINDOW_MILLIS,
      flaky_tests,
    },
  },
  time::Timestamp,
};

const FAILING: &str = "TAP version 13\nok 1 - adds\nnot ok 2 - subtracts\n  ---\n  location: 'src/math.test.mjs:8:1'\n  \
                       error: |-\n    8 !== 2\n  ...\n1..2\n";
const PASSING: &str = "TAP version 13\nok 1 - adds\nok 2 - subtracts\n1..2\n";
const DAY: i64 = 24 * 60 * 60 * 1000;

fn slug(value: &str) -> Slug {
  Slug::new(value).expect("slug")
}

fn key(suite: &str, test: &str) -> TestKey {
  TestKey::new(slug(suite), test).expect("test key")
}

fn run(report: &str) -> SuiteRun {
  let report = test_report::read_report(test_report::ReportFormat::Tap, report).expect("a TAP report");
  SuiteRun {
    id: TestRunId::FIRST,
    suite: "unit".to_owned(),
    filter: None,
    exit_code: Some(if report.passed() { 0 } else { 1 }),
    duration_ms: 10,
    end: SuiteRunEnd::Read { report },
    artifacts: Vec::new(),
    artifact_problems: Vec::new(),
  }
}

fn flaky(test: &TestKey, run: RunId, day: i64) -> FlakyOutcome {
  FlakyOutcome {
    test: test.clone(),
    kind: FlakyKind::Flaky,
    run,
    at: Timestamp::from_unix_millis(day * DAY),
  }
}

#[test]
fn flaky_tests_a_test_flaky_in_three_runs_within_two_weeks_is_watched() {
  assert_eq!((WATCH_AFTER, WATCH_WINDOW_MILLIS), (3, 14 * DAY));
  let subtracts = key("unit", "subtracts");
  let (first, second, third) = (RunId::generate(), RunId::generate(), RunId::generate());
  let now = Timestamp::from_unix_millis(30 * DAY);
  // Twice in one run counts once.
  let mut outcomes = vec![
    flaky(&subtracts, first, 20),
    flaky(&subtracts, first, 21),
    flaky(&subtracts, second, 25),
  ];
  let seen = flaky_tests(&outcomes, &[], now);
  assert_eq!(seen.len(), 1);
  assert_eq!((seen[0].standing, seen[0].flaky_runs), (Standing::Seen, 2));
  // A run older than the window does not count.
  let old = flaky_tests(
    &[flaky(&subtracts, third, 15), outcomes[0].clone(), outcomes[2].clone()],
    &[],
    now,
  );
  assert_eq!((old[0].standing, old[0].flaky_runs), (Standing::Seen, 2));

  outcomes.push(flaky(&subtracts, third, 29));
  let watched = flaky_tests(&outcomes, &[], now);
  assert_eq!((watched[0].standing, watched[0].flaky_runs), (Standing::Watched, 3));
  assert_eq!(watched[0].last_flaky, Some(Timestamp::from_unix_millis(29 * DAY)));

  // Quarantined first, whatever its count; then watched; then the others.
  let adds = key("unit", "adds");
  let lint = key("lint", "format");
  outcomes.push(flaky(&adds, first, 29));
  let since = Timestamp::from_unix_millis(29 * DAY);
  let listed = flaky_tests(&outcomes, &[(lint.clone(), since)], now);
  let order: Vec<(&TestKey, Standing)> = listed.iter().map(|test| (&test.test, test.standing)).collect();
  assert_eq!(
    order,
    [
      (&lint, Standing::Quarantined { since }),
      (&subtracts, Standing::Watched),
      (&adds, Standing::Seen)
    ]
  );
  assert_eq!(TestKey::new(slug("unit"), "  "), Err(TestKeyError::Blank));
}

#[test]
fn flaky_tests_a_quarantined_test_that_fails_again_does_not_fail_the_step() {
  let quarantine = Quarantine::new([key("unit", "subtracts")]);
  let first = (run(FAILING), "t1/result.json".to_owned());
  let quarantined = SuiteOutcome::settle(slug("unit"), TestLayer::Unit, first, &[run(FAILING)], &quarantine);
  assert!(quarantined.passed(), "{quarantined:?}");
  assert!(quarantined.failed.is_empty());
  assert_eq!(quarantined.quarantined[0].name, "subtracts");
  // Quarantine names a test of one suite: the same name in another suite still fails.
  let other = SuiteOutcome::settle(
    slug("web"),
    TestLayer::Unit,
    (run(FAILING), "t2/result.json".to_owned()),
    &[run(FAILING)],
    &quarantine,
  );
  assert!(!other.passed());
  // One that passes again is flaky, quarantined or not.
  let passed_again = SuiteOutcome::settle(
    slug("unit"),
    TestLayer::Unit,
    (run(FAILING), "t3/result.json".to_owned()),
    &[run(PASSING)],
    &quarantine,
  );
  assert_eq!((passed_again.flaky.len(), passed_again.quarantined.len()), (1, 0));

  let result = TestStepResult {
    reached: vec![(TestLayer::Unit, vec![quarantined.clone()])],
    not_reached: Vec::new(),
    left_out: Vec::new(),
  };
  assert!(result.passed() && result.failure().is_none());
  let summary = result.summary();
  assert!(
    summary.contains("Quarantined by the person, so they do not fail the step"),
    "{summary}"
  );
  assert!(
    summary.contains("- `subtracts` in `unit`: failed at src/math.test.mjs:8"),
    "{summary}"
  );
  let (run_id, at) = (RunId::generate(), Timestamp::from_unix_millis(5));
  let kept = result.flaky_outcomes(run_id, at);
  assert_eq!(
    kept,
    [FlakyOutcome {
      test: key("unit", "subtracts"),
      kind: FlakyKind::QuarantinedFailure,
      run: run_id,
      at
    }]
  );

  // When the step fails for another suite, the writing step reads only that suite's failures.
  let failing = TestStepResult {
    reached: vec![(TestLayer::Unit, vec![quarantined, other])],
    not_reached: Vec::new(),
    left_out: Vec::new(),
  };
  let report = failing.failure_report();
  assert!(
    report.starts_with("The Test step failed at the unit layer: 1 tests failed in 1 suites."),
    "{report}"
  );
  assert!(
    !report.contains("Quarantined") && !report.contains("`unit`"),
    "{report}"
  );
}
