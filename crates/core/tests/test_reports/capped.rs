//! Captured output is capped to its end, messages to their start, and a failure's excerpt to about 20 lines.

use orchester_core::test_report::{
  EXCERPT_LINE_CHARS, EXCERPT_LINES, MESSAGE_CAP_BYTES, OUTPUT_CAP_BYTES, ReportFormat,
};

use super::{read, test};

fn cargo_failure(stdout: &str) -> String {
  let line = serde_json::json!({ "type": "test", "name": "tests::floods", "event": "failed", "stdout": stdout });
  format!(
    "{{ \"type\": \"suite\", \"event\": \"started\", \"test_count\": 1 }}\n{line}\n\
     {{ \"type\": \"suite\", \"event\": \"failed\", \"passed\": 0, \"failed\": 1 }}\n"
  )
}

#[test]
fn test_reports_captured_output_keeps_its_last_bytes() {
  let printed = (0..20_000).map(|n| format!("line {n}\n")).collect::<String>();
  let stdout = format!("{printed}thread 'tests::floods' panicked at src/lib.rs:4:5:\nboom\n");
  let report = read(ReportFormat::CargoJson, &cargo_failure(&stdout));

  let output = test(&report, "floods").output().expect("the test printed");
  assert!(output.text().len() <= OUTPUT_CAP_BYTES);
  assert_eq!(output.omitted_bytes() + output.text().len(), stdout.len());
  assert!(output.text().ends_with("boom\n"), "keeps the end, where the failure is");
}

#[test]
fn test_reports_failure_excerpts_are_at_most_twenty_short_lines() {
  let message = (0..200).map(|n| format!("detail {n}")).collect::<Vec<_>>().join("\n");
  let stdout = format!("thread 'tests::floods' panicked at src/lib.rs:4:5:\n{message}\n");
  let report = read(ReportFormat::CargoJson, &cargo_failure(&stdout));

  let excerpt = test(&report, "floods").excerpt().expect("a failure has an excerpt");
  assert_eq!(EXCERPT_LINES, 20);
  assert_eq!(excerpt.lines().count(), EXCERPT_LINES);
  assert!(excerpt.starts_with("detail 0\n"));

  let wide = format!(
    "thread 'tests::floods' panicked at src/lib.rs:4:5:\n{}\n",
    "x".repeat(10_000)
  );
  let report = read(ReportFormat::CargoJson, &cargo_failure(&wide));
  let excerpt = test(&report, "floods").excerpt().expect("a failure has an excerpt");
  assert!(excerpt.chars().count() <= EXCERPT_LINE_CHARS + 1);
}

#[test]
fn test_reports_messages_are_capped_to_their_start() {
  let stdout = format!(
    "thread 'tests::floods' panicked at src/lib.rs:4:5:\nstart {}\n",
    "é".repeat(20_000)
  );
  let report = read(ReportFormat::CargoJson, &cargo_failure(&stdout));

  let message = test(&report, "floods").message().expect("the panic's message");
  assert!(message.starts_with("start "));
  assert!(message.len() <= MESSAGE_CAP_BYTES + '…'.len_utf8());
}

#[test]
fn test_reports_a_failure_without_a_message_excerpts_the_end_of_its_output() {
  let printed = (0..50).map(|n| format!("step {n}\n")).collect::<String>();
  let xml = format!(
    "<testsuites><testsuite name=\"s\"><testcase name=\"silent\"><failure/><system-out>{printed}</system-out>\
     </testcase></testsuite></testsuites>"
  );
  let report = read(ReportFormat::JunitXml, &xml);

  let excerpt = test(&report, "silent").excerpt().expect("a failure has an excerpt");
  assert_eq!(excerpt.lines().count(), EXCERPT_LINES);
  assert!(
    excerpt.starts_with("step 30\n") && excerpt.ends_with("step 49"),
    "{excerpt}"
  );
}
