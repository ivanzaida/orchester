//! Reports that are cut off, malformed or empty give a typed error, never a report where everything passed.

use orchester_core::test_report::{Position, ReportFormat, TestReportError, Totals, read_report};

use super::{CARGO, CARGO_ABORTED, NODE_JUNIT, NODE_TAP, PLAYWRIGHT, VITEST};

const FORMATS: [ReportFormat; 5] = [
  ReportFormat::CargoJson,
  ReportFormat::JunitXml,
  ReportFormat::VitestJson,
  ReportFormat::PlaywrightJson,
  ReportFormat::Tap,
];

fn error(format: ReportFormat, text: &str) -> TestReportError {
  match read_report(format, text) {
    Ok(report) => panic!("a broken {format} report must not read; read {}", report.totals()),
    Err(error) => error,
  }
}

fn truncated(format: ReportFormat, text: &str) -> (String, Totals) {
  match error(format, text) {
    TestReportError::Truncated { detail, read, .. } => (detail, read),
    other => panic!("the {format} report is cut off, not {other:?}"),
  }
}

fn malformed_at(format: ReportFormat, text: &str) -> Position {
  match error(format, text) {
    TestReportError::Malformed { at, .. } => at,
    other => panic!("the {format} report is malformed, not {other:?}"),
  }
}

#[test]
fn test_reports_empty_reports_are_empty_in_every_format() {
  for format in FORMATS {
    assert_eq!(error(format, " \n\t\n"), TestReportError::Empty { format });
  }
}

#[test]
fn test_reports_text_that_is_no_report_is_malformed_in_every_format() {
  for format in FORMATS {
    assert_eq!(malformed_at(format, "hello\n").line, 1, "{format}");
  }
}

#[test]
fn test_reports_cargo_json_a_test_binary_that_stopped_is_cut_off_with_what_was_read() {
  let (detail, read) = truncated(ReportFormat::CargoJson, CARGO_ABORTED);

  assert!(
    detail.contains("tests::recurses"),
    "names the test that never finished: {detail}"
  );
  assert_eq!(
    read,
    Totals {
      passed: 1,
      failed: 1,
      skipped: 0,
      errored: 0
    }
  );
}

#[test]
fn test_reports_cargo_json_cut_off_lines_and_missing_suite_ends_are_truncated() {
  let without_end = CARGO.trim_end().rsplit_once('\n').map_or("", |(rest, _)| rest);
  let (_, read) = truncated(ReportFormat::CargoJson, without_end);
  assert_eq!(read.tests(), 5);

  let cut_line = &CARGO.trim_end()[..CARGO.trim_end().len() - 30];
  let (detail, _) = truncated(ReportFormat::CargoJson, cut_line);
  assert!(detail.contains("cut off"), "{detail}");

  // A second suite starting while the first never ended: its binary stopped.
  let restarted = format!("{without_end}\n{}", CARGO);
  truncated(ReportFormat::CargoJson, &restarted);
}

#[test]
fn test_reports_cargo_json_a_broken_line_in_the_middle_is_malformed_where_it_is() {
  let mut lines = CARGO.lines().collect::<Vec<_>>();
  lines[3] = "{ \"type\": \"test\", \"event\": ";
  let text = lines.join("\n");

  assert_eq!(malformed_at(ReportFormat::CargoJson, &text).line, 4);
}

#[test]
fn test_reports_junit_xml_cut_off_and_malformed_documents_are_errors() {
  let cut = &NODE_JUNIT[..NODE_JUNIT
    .find("\t<testcase name=\"downloads")
    .unwrap_or(NODE_JUNIT.len())];
  truncated(ReportFormat::JunitXml, cut);

  assert_eq!(
    malformed_at(ReportFormat::JunitXml, "<?xml version=\"1.0\"?>\n<html/>").line,
    2
  );
  let nameless = "<testsuites>\n<testsuite name=\"a\">\n<testcase time=\"1\"/>\n</testsuite>\n</testsuites>";
  assert_eq!(malformed_at(ReportFormat::JunitXml, nameless).line, 3);
}

#[test]
fn test_reports_json_reports_cut_off_or_of_another_shape_are_errors() {
  for (format, text) in [
    (ReportFormat::VitestJson, VITEST),
    (ReportFormat::PlaywrightJson, PLAYWRIGHT),
  ] {
    truncated(format, &text[..text.len() / 2]);
    malformed_at(format, "{\"numTotalTests\": 0}");
  }
  let unknown_status = VITEST.replace(r#""status":"todo""#, r#""status":"exploded""#);
  malformed_at(ReportFormat::VitestJson, &unknown_status);
}

#[test]
fn test_reports_tap_runs_that_stop_short_of_their_plan_are_truncated() {
  let short = "TAP version 13\n1..4\nok 1 - adds\nnot ok 2 - subtracts\n";
  let (detail, read) = truncated(ReportFormat::Tap, short);
  assert!(detail.contains("plan names 4 tests"), "{detail}");
  assert_eq!(
    read,
    Totals {
      passed: 1,
      failed: 1,
      skipped: 0,
      errored: 0
    }
  );

  let without_plan = NODE_TAP.replace("1..4\n", "");
  let (detail, _) = truncated(ReportFormat::Tap, &without_plan);
  assert!(detail.contains("no plan"), "{detail}");

  let open_yaml = &NODE_TAP[..NODE_TAP.find("  code: 'ERR_ASSERTION'").unwrap_or(NODE_TAP.len())];
  truncated(ReportFormat::Tap, open_yaml);
}

#[test]
fn test_reports_tap_bail_out_and_too_many_tests_are_errors() {
  let bailed = "TAP version 13\n1..3\nok 1 - adds\nBail out! database is gone\n";
  assert_eq!(
    error(ReportFormat::Tap, bailed),
    TestReportError::BailedOut {
      reason: "database is gone".to_owned(),
      read: Totals {
        passed: 1,
        failed: 0,
        skipped: 0,
        errored: 0
      }
    }
  );

  let too_many = "1..1\nok 1 - adds\nok 2 - subtracts\n";
  assert_eq!(malformed_at(ReportFormat::Tap, too_many).line, 1);
}
