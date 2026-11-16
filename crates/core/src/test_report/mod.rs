//! Test reports read into one result shape (`docs/architecture/agent-testing.md`, "Test suite" and "The Test
//! step"): a test suite writes its report in one of the [`ReportFormat`]s, and [`read_report`] turns it into a
//! [`TestReport`] — each test with its suite path, outcome, duration, message, file and line where the format gives
//! them, its captured output (capped) and, for a failure, a short excerpt — or into a [`TestReportError`] that says
//! what was readable. A report that is cut off or cannot be read is never taken for one where every test passed.

mod answer;
mod case;
mod error;
mod excerpt;
mod junit;
mod libtest;
mod playwright;
mod report;
mod run;
mod tap;
mod vitest;

use std::fmt;

pub use answer::{MAX_FAILURES_SHOWN, TEST_OUTPUT_BYTES};
pub use case::{
  Attachment, CapturedOutput, EXCERPT_LINE_CHARS, EXCERPT_LINES, MESSAGE_CAP_BYTES, OUTPUT_CAP_BYTES, SourceLocation,
  TestCase, TestOutcome,
};
pub use error::{Position, TestReportError};
pub use report::{RunError, TestReport, Totals};
pub use run::{LAST_LINES, StoredArtifact, SuiteRun, SuiteRunEnd, TestRunId, last_lines, test_name};
use serde::{Deserialize, Serialize};

use crate::resource;

/// The report formats Orchester reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportFormat {
  /// libtest's JSON lines: `cargo test -- -Z unstable-options --format json --report-time`.
  CargoJson,
  /// JUnit XML, as most test runners write it.
  JunitXml,
  /// vitest's `json` reporter.
  VitestJson,
  /// Playwright's `json` reporter.
  PlaywrightJson,
  /// TAP version 13.
  Tap,
}

impl fmt::Display for ReportFormat {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Self::CargoJson => "cargo json",
      Self::JunitXml => "JUnit XML",
      Self::VitestJson => "vitest json",
      Self::PlaywrightJson => "Playwright json",
      Self::Tap => "TAP",
    })
  }
}

/// The format a test suite says its report has.
impl From<resource::ReportFormat> for ReportFormat {
  fn from(format: resource::ReportFormat) -> Self {
    match format {
      resource::ReportFormat::CargoJson => Self::CargoJson,
      resource::ReportFormat::JunitXml => Self::JunitXml,
      resource::ReportFormat::VitestJson => Self::VitestJson,
      resource::ReportFormat::PlaywrightJson => Self::PlaywrightJson,
      resource::ReportFormat::Tap => Self::Tap,
    }
  }
}

/// Reads a whole report of `format`.
pub fn read_report(format: ReportFormat, text: &str) -> Result<TestReport, TestReportError> {
  let text = text.strip_prefix('\u{feff}').unwrap_or(text);
  if text.trim().is_empty() {
    return Err(TestReportError::Empty { format });
  }
  match format {
    ReportFormat::CargoJson => libtest::read(text),
    ReportFormat::JunitXml => junit::read(text),
    ReportFormat::VitestJson => vitest::read(text),
    ReportFormat::PlaywrightJson => playwright::read(text),
    ReportFormat::Tap => tap::read(text),
  }
}
