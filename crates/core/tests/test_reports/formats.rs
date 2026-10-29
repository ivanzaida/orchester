use std::time::Duration;

use orchester_core::test_report::{ReportFormat, SourceLocation, TestOutcome, Totals};

use super::{CARGO, NODE_JUNIT, NODE_TAP, PLAYWRIGHT, PYTEST_JUNIT, VITEST, read, test};

fn at(file: &str, line: Option<u32>) -> Option<SourceLocation> {
  Some(SourceLocation {
    file: file.to_owned(),
    line,
  })
}

/// Whether a report's duration is `seconds`, give or take a microsecond of float rounding.
fn lasted(duration: Option<Duration>, seconds: f64) -> bool {
  duration.is_some_and(|duration| (duration.as_secs_f64() - seconds).abs() < 1e-6)
}

#[test]
fn test_reports_cargo_json_reads_panics_with_their_file_and_line() {
  let report = read(ReportFormat::CargoJson, CARGO);

  assert_eq!(
    report.totals(),
    Totals {
      passed: 2,
      failed: 2,
      skipped: 1,
      errored: 0
    }
  );
  assert!(!report.passed());
  let subtracts = test(&report, "subtracts");
  assert_eq!(subtracts.suite(), ["tests"]);
  assert_eq!(subtracts.outcome(), TestOutcome::Failed);
  assert_eq!(subtracts.location().cloned(), at(r"src\lib.rs", Some(21)));
  assert_eq!(
    subtracts.message(),
    Some("assertion `left == right` failed\n  left: 8\n right: 2")
  );
  assert!(lasted(subtracts.duration(), 0.000_082));
  assert!(
    subtracts
      .output()
      .is_some_and(|output| output.text().starts_with("subtracting 5 - 3"))
  );
  assert!(
    subtracts
      .excerpt()
      .is_some_and(|excerpt| excerpt.starts_with("assertion `left == right` failed"))
  );

  let overflow = test(&report, "rejects_overflow");
  assert_eq!(overflow.location().cloned(), at(r"src\lib.rs", Some(30)));
  assert_eq!(
    overflow.message(),
    Some(r"test did not panic as expected at src\lib.rs:30:6")
  );

  assert_eq!(test(&report, "deep").suite(), ["tests", "nested"]);
  let downloads = test(&report, "downloads");
  assert_eq!(
    (downloads.outcome(), downloads.message()),
    (TestOutcome::Skipped, Some("needs a network"))
  );
  assert_eq!(test(&report, "adds").excerpt(), None);
}

#[test]
fn test_reports_junit_xml_reads_node_and_pytest_reports() {
  let node = read(ReportFormat::JunitXml, NODE_JUNIT);
  assert_eq!(
    node.totals(),
    Totals {
      passed: 1,
      failed: 1,
      skipped: 2,
      errored: 0
    }
  );
  let subtracts = test(&node, "subtracts numbers");
  assert_eq!(subtracts.suite(), ["test"]);
  assert_eq!(
    subtracts.location().cloned(),
    at(r"C:\work\calc\test\math.test.mjs", None)
  );
  assert!(
    subtracts
      .message()
      .is_some_and(|message| message.starts_with("Expected values to be strictly equal:"))
  );
  // node writes a `todo` test as a `<skipped>` beside its failure: an expected failure.
  assert_eq!(test(&node, "divides by zero").message(), Some("decide the rule"));
  assert!(node.run_errors().is_empty());

  let pytest = read(ReportFormat::JunitXml, PYTEST_JUNIT);
  assert_eq!(
    pytest.totals(),
    Totals {
      passed: 1,
      failed: 1,
      skipped: 1,
      errored: 1
    }
  );
  let subtracts = test(&pytest, "test_subtracts");
  assert_eq!(subtracts.suite(), ["pytest", "tests.test_math"]);
  assert_eq!(subtracts.location().cloned(), at("tests/test_math.py", Some(7)));
  assert!(
    subtracts
      .message()
      .is_some_and(|message| message.starts_with("assert 8 == 2"))
  );
  assert_eq!(
    subtracts.output().map(|output| output.text()),
    Some("subtracting 5 - 3\n")
  );
  assert_eq!(test(&pytest, "test_downloads").message(), Some("needs a network"));
  assert_eq!(test(&pytest, "test_connects").outcome(), TestOutcome::Errored);
  assert!(pytest.run_errors().is_empty());
}

#[test]
fn test_reports_junit_xml_suite_counts_beyond_its_cases_are_a_run_error() {
  let report = read(
    ReportFormat::JunitXml,
    &PYTEST_JUNIT.replace(r#"errors="1""#, r#"errors="2""#),
  );

  assert_eq!(report.run_errors().len(), 1);
  assert!(
    report.run_errors()[0]
      .message()
      .contains("reports 3 failures and errors")
  );
  assert!(!report.passed());
}

#[test]
fn test_reports_vitest_json_reads_tests_and_files_that_failed_to_load() {
  let report = read(ReportFormat::VitestJson, VITEST);

  assert_eq!(
    report.totals(),
    Totals {
      passed: 2,
      failed: 1,
      skipped: 2,
      errored: 0
    }
  );
  let file = "/work/calc/src/math.test.ts";
  let adds = test(&report, "adds two numbers");
  assert_eq!(adds.suite(), [file, "math", "add"]);
  assert_eq!(adds.location().cloned(), at(file, Some(6)));
  let subtracts = test(&report, "subtracts");
  // No `location` without `includeTaskLocation`: the line is the stack frame in the test file.
  assert_eq!(subtracts.location().cloned(), at(file, Some(12)));
  assert!(
    subtracts
      .message()
      .is_some_and(|message| message.starts_with("AssertionError: expected 8 to be 2"))
  );
  assert_eq!(test(&report, "divides").outcome(), TestOutcome::Skipped);

  let [load] = report.run_errors() else {
    panic!("one file failed to load")
  };
  assert!(load.message().starts_with("Failed to load url ./missing"));
  assert_eq!(load.location().cloned(), at("/work/calc/src/broken.test.ts", None));
  assert!(!report.passed());
}

#[test]
fn test_reports_vitest_json_a_failed_run_without_failed_tests_is_a_run_error() {
  let text = r#"{"success":false,"testResults":[{"name":"/a.test.ts","message":"","assertionResults":[
    {"ancestorTitles":[],"title":"works","status":"passed","failureMessages":[]}]}]}"#;
  let report = read(ReportFormat::VitestJson, text);

  assert_eq!(report.totals().passed, 1);
  assert_eq!(report.run_errors().len(), 1);
  assert!(!report.passed());
}

#[test]
fn test_reports_playwright_json_reads_failures_with_their_attachments() {
  let report = read(ReportFormat::PlaywrightJson, PLAYWRIGHT);

  assert_eq!(
    report.totals(),
    Totals {
      passed: 1,
      failed: 1,
      skipped: 1,
      errored: 0
    }
  );
  let empty = test(&report, "shows an empty cart");
  assert_eq!(empty.suite(), ["chromium", "cart.spec.ts"]);
  assert_eq!(empty.location().cloned(), at("cart.spec.ts", Some(3)));
  assert!(lasted(empty.duration(), 0.812));

  let pays = test(&report, "pays with a card");
  assert_eq!(pays.suite(), ["chromium", "cart.spec.ts", "checkout"]);
  assert_eq!(pays.outcome(), TestOutcome::Failed);
  assert_eq!(pays.location().cloned(), at("/work/shop/e2e/cart.spec.ts", Some(14)));
  let message = pays.message().unwrap_or_default();
  assert!(
    message.starts_with("Error: expect(locator).toHaveText(expected) failed"),
    "colours removed: {message}"
  );
  assert!(message.contains(r#"Received: "Card declined""#));
  let attachments = pays
    .attachments()
    .iter()
    .map(|attachment| (attachment.name.as_str(), attachment.path.as_str()));
  assert_eq!(
    attachments.collect::<Vec<_>>(),
    [
      (
        "screenshot",
        "/work/shop/test-results/cart-checkout-pays-with-a-card-chromium/test-failed-1.png"
      ),
      (
        "trace",
        "/work/shop/test-results/cart-checkout-pays-with-a-card-chromium/trace.zip"
      ),
    ]
  );
  assert_eq!(
    pays.output().map(|output| output.text()),
    Some("paying 12.50\nwarning: slow gateway\n")
  );
  assert_eq!(test(&report, "applies a coupon").outcome(), TestOutcome::Skipped);
}

#[test]
fn test_reports_playwright_json_global_errors_are_run_errors() {
  let text = r#"{"suites":[],"errors":[{"message":"Error: Cannot find module './config'",
    "location":{"file":"/work/shop/playwright.config.ts","line":2,"column":1}}]}"#;
  let report = read(ReportFormat::PlaywrightJson, text);

  assert!(report.tests().is_empty());
  let [error] = report.run_errors() else {
    panic!("one global error")
  };
  assert_eq!(
    error.location().cloned(),
    at("/work/shop/playwright.config.ts", Some(2))
  );
  assert!(!report.passed());
}

#[test]
fn test_reports_tap_reads_node_test_runner_output() {
  let report = read(ReportFormat::Tap, NODE_TAP);

  assert_eq!(
    report.totals(),
    Totals {
      passed: 1,
      failed: 1,
      skipped: 2,
      errored: 0
    }
  );
  let subtracts = test(&report, "subtracts numbers");
  assert_eq!(subtracts.outcome(), TestOutcome::Failed);
  assert!(
    subtracts
      .location()
      .is_some_and(|location| location.file.ends_with("math.test.mjs"))
  );
  assert_eq!(subtracts.location().and_then(|location| location.line), Some(8));
  assert_eq!(
    subtracts.message(),
    Some("Expected values to be strictly equal:\n8 !== 2")
  );
  assert!(lasted(subtracts.duration(), 0.001_621));
  assert_eq!(test(&report, "downloads a file").message(), Some("needs a network"));
  let todo = test(&report, "divides by zero");
  assert_eq!(
    (todo.outcome(), todo.message()),
    (TestOutcome::Skipped, Some("decide the rule"))
  );
}

#[test]
fn test_reports_tap_reads_node_tap_diagnostics_with_an_at_mapping() {
  let text = "TAP version 13\n1..2\nok 1 - adds\nnot ok 2 - subtracts\n  ---\n  message: 'expected 8 to equal 2'\n  \
              at:\n    line: 12\n    column: 5\n    file: test/math.js\n  ...\n";
  let report = read(ReportFormat::Tap, text);

  let subtracts = test(&report, "subtracts");
  assert_eq!(subtracts.message(), Some("expected 8 to equal 2"));
  assert_eq!(subtracts.location().cloned(), at("test/math.js", Some(12)));
  assert_eq!(subtracts.excerpt(), Some("expected 8 to equal 2"));
}
