//! vitest's `json` reporter (Jest's shape): `testResults` per test file, each with its `assertionResults`. A test's
//! `location` is there when vitest runs with `includeTaskLocation`; otherwise the line comes from the failure's
//! stack where it names the test file. A file that failed to load has a `message` and no tests.

use serde::Deserialize;

use super::{
  ReportFormat, TestReportError,
  case::{CaseParts, SourceLocation, TestCase, TestOutcome, seconds},
  report::{RunError, TestReport},
};

const FORMAT: ReportFormat = ReportFormat::VitestJson;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Report {
  test_results: Vec<FileResult>,
  #[serde(default)]
  success: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileResult {
  name: String,
  #[serde(default)]
  message: Option<String>,
  assertion_results: Vec<Assertion>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Assertion {
  #[serde(default)]
  ancestor_titles: Vec<String>,
  title: String,
  status: Status,
  #[serde(default)]
  duration: Option<f64>,
  #[serde(default)]
  failure_messages: Vec<String>,
  #[serde(default)]
  location: Option<Location>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Status {
  Passed,
  Failed,
  Skipped,
  Pending,
  Todo,
  Disabled,
}

#[derive(Deserialize)]
struct Location {
  line: u32,
}

pub(super) fn read(text: &str) -> Result<TestReport, TestReportError> {
  let report = serde_json::from_str::<Report>(text).map_err(|error| TestReportError::json(FORMAT, &error))?;
  let mut tests = Vec::new();
  let mut run_errors = Vec::new();
  for file in report.test_results {
    if let Some(message) = file.message.as_deref().filter(|message| !message.trim().is_empty()) {
      run_errors.push(RunError::new(
        message,
        Some(SourceLocation {
          file: file.name.clone(),
          line: None,
        }),
      ));
    }
    tests.extend(
      file
        .assertion_results
        .into_iter()
        .map(|assertion| case(&file.name, assertion)),
    );
  }
  let failed = !run_errors.is_empty() || tests.iter().any(|test: &TestCase| test.outcome().is_failure());
  // Errors outside every test (an unhandled rejection) fail vitest's run without failing a test.
  if report.success == Some(false) && !failed {
    run_errors.push(RunError::new(
      "vitest reported the run as failed, but no test or file failed",
      None,
    ));
  }
  Ok(TestReport::new(FORMAT, tests, run_errors))
}

fn case(file: &str, assertion: Assertion) -> TestCase {
  let outcome = match assertion.status {
    Status::Passed => TestOutcome::Passed,
    Status::Failed => TestOutcome::Failed,
    Status::Skipped | Status::Pending | Status::Todo | Status::Disabled => TestOutcome::Skipped,
  };
  let mut suite = vec![file.to_owned()];
  suite.extend(assertion.ancestor_titles);
  let mut parts = CaseParts::new(suite, assertion.title, outcome);
  parts.duration = assertion.duration.map(|ms| ms / 1000.0).and_then(seconds);
  let message = assertion.failure_messages.join("\n");
  let line = assertion
    .location
    .map(|location| location.line)
    .or_else(|| line_in_stack(&message, file));
  parts.location = Some(SourceLocation {
    file: file.to_owned(),
    line,
  });
  parts.message = Some(message);
  parts.finish()
}

/// The line of the first stack frame in `file`: `at …/math.test.ts:12:5`.
fn line_in_stack(message: &str, file: &str) -> Option<u32> {
  let (_, after) = message.split_once(&format!("{file}:"))?;
  let digits = after.split(|c: char| !c.is_ascii_digit()).next()?;
  digits.parse().ok()
}
