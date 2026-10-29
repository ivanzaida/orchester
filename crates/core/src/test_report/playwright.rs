//! Playwright's `json` reporter: nested `suites` (a file, then its `describe` blocks) with `specs`, each run once
//! per project as a test with one result per attempt. The last attempt decides the message and where it failed; the
//! attachments with a path (screenshots, traces, videos) are kept from every attempt. The report's own `errors`
//! (a config that failed to load, an error outside every test) are run errors.

use base64::Engine as _;
use serde::Deserialize;

use super::{
  ReportFormat, TestReportError,
  case::{Attachment, CaseParts, SourceLocation, TestCase, TestOutcome, seconds},
  report::{RunError, TestReport},
};

const FORMAT: ReportFormat = ReportFormat::PlaywrightJson;

#[derive(Deserialize)]
struct Report {
  suites: Vec<Suite>,
  #[serde(default)]
  errors: Vec<Error>,
}

#[derive(Deserialize)]
struct Suite {
  title: String,
  #[serde(default)]
  specs: Vec<Spec>,
  #[serde(default)]
  suites: Vec<Suite>,
}

#[derive(Deserialize)]
struct Spec {
  title: String,
  file: String,
  #[serde(default)]
  line: Option<u32>,
  tests: Vec<Test>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Test {
  #[serde(default)]
  project_name: String,
  status: TestStatus,
  results: Vec<RunResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum TestStatus {
  Expected,
  Unexpected,
  Flaky,
  Skipped,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunResult {
  status: ResultStatus,
  #[serde(default)]
  duration: Option<f64>,
  #[serde(default)]
  errors: Vec<Error>,
  #[serde(default)]
  stdout: Vec<Chunk>,
  #[serde(default)]
  stderr: Vec<Chunk>,
  #[serde(default)]
  attachments: Vec<RawAttachment>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum ResultStatus {
  Passed,
  Failed,
  TimedOut,
  Skipped,
  Interrupted,
}

#[derive(Deserialize)]
struct Error {
  #[serde(default)]
  message: Option<String>,
  #[serde(default)]
  location: Option<Location>,
}

#[derive(Deserialize)]
struct Location {
  file: String,
  line: u32,
}

/// Printed text, or bytes as base64 when they were not text.
#[derive(Deserialize)]
#[serde(untagged)]
enum Chunk {
  Text { text: String },
  Buffer { buffer: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawAttachment {
  name: String,
  content_type: String,
  #[serde(default)]
  path: Option<String>,
}

pub(super) fn read(text: &str) -> Result<TestReport, TestReportError> {
  let report = serde_json::from_str::<Report>(text).map_err(|error| TestReportError::json(FORMAT, &error))?;
  let mut tests = Vec::new();
  for suite in &report.suites {
    read_suite(suite, &[], &mut tests);
  }
  let run_errors = report.errors.iter().map(|error| error.run_error()).collect();
  Ok(TestReport::new(FORMAT, tests, run_errors))
}

fn read_suite(suite: &Suite, path: &[&str], tests: &mut Vec<TestCase>) {
  let mut path = path.to_vec();
  path.push(&suite.title);
  for spec in &suite.specs {
    tests.extend(spec.tests.iter().map(|test| case(&path, spec, test)));
  }
  for inner in &suite.suites {
    read_suite(inner, &path, tests);
  }
}

fn case(path: &[&str], spec: &Spec, test: &Test) -> TestCase {
  let last = test.results.last();
  let outcome = match (&test.status, last.map(|result| &result.status)) {
    (TestStatus::Expected | TestStatus::Flaky, _) => TestOutcome::Passed,
    (TestStatus::Skipped, _) => TestOutcome::Skipped,
    (TestStatus::Unexpected, Some(ResultStatus::Interrupted)) => TestOutcome::Errored,
    (TestStatus::Unexpected, _) => TestOutcome::Failed,
  };
  let project = Some(test.project_name.as_str()).filter(|name| !name.is_empty());
  let suite = project
    .into_iter()
    .chain(path.iter().copied())
    .map(str::to_owned)
    .collect();
  let mut parts = CaseParts::new(suite, spec.title.clone(), outcome);
  parts.duration = last
    .and_then(|result| result.duration)
    .map(|ms| ms / 1000.0)
    .and_then(seconds);
  let errors = last.map_or(&[][..], |result| &result.errors);
  let messages = errors
    .iter()
    .filter_map(|error| error.message.as_deref())
    .collect::<Vec<_>>();
  parts.message = (!messages.is_empty()).then(|| messages.join("\n"));
  let failed_at = errors.iter().find_map(|error| error.location.as_ref());
  parts.location = Some(match failed_at {
    Some(location) => SourceLocation {
      file: location.file.clone(),
      line: Some(location.line),
    },
    None => SourceLocation {
      file: spec.file.clone(),
      line: spec.line,
    },
  });
  for result in &test.results {
    for chunk in result.stdout.iter().chain(&result.stderr) {
      chunk.write_to(&mut parts.output);
    }
    parts
      .attachments
      .extend(result.attachments.iter().filter_map(RawAttachment::kept));
  }
  parts.finish()
}

impl Error {
  fn run_error(&self) -> RunError {
    let location = self.location.as_ref().map(|location| SourceLocation {
      file: location.file.clone(),
      line: Some(location.line),
    });
    RunError::new(self.message.as_deref().unwrap_or_default(), location)
  }
}

impl Chunk {
  fn write_to(&self, output: &mut String) {
    match self {
      Self::Text { text } => output.push_str(text),
      Self::Buffer { buffer } => match base64::engine::general_purpose::STANDARD.decode(buffer) {
        Ok(bytes) => output.push_str(&String::from_utf8_lossy(&bytes)),
        Err(_) => output.push_str("[output that is not valid base64]\n"),
      },
    }
  }
}

impl RawAttachment {
  /// An attachment that is a file; one carried inline in the report (`body`) has no path to hand on.
  fn kept(&self) -> Option<Attachment> {
    let path = self.path.clone()?;
    Some(Attachment {
      name: self.name.clone(),
      content_type: self.content_type.clone(),
      path,
    })
  }
}
