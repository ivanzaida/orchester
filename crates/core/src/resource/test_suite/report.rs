//! What a test suite's program reports and where Orchester reads it.

use std::fmt;

use super::{SuitePath, TestSuiteError};

/// The shape of a suite's report; each is read into one result shape (agent testing task 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReportFormat {
  /// libtest's JSON lines (`cargo test -- -Z unstable-options --format json`), printed.
  CargoJson,
  /// A JUnit XML file, as nextest, pytest and most runners write it.
  JunitXml,
  /// Vitest's `--reporter=json`, printed or written with `--outputFile`.
  VitestJson,
  /// Playwright's `--reporter=json`, printed or written to `PLAYWRIGHT_JSON_OUTPUT_NAME`.
  PlaywrightJson,
  /// The Test Anything Protocol, printed.
  Tap,
}

/// Where a format's report goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportOutput {
  /// The program's standard output.
  Printed,
  /// A file the program writes.
  File,
  /// Either, as the suite says.
  PrintedOrFile,
}

impl ReportFormat {
  pub const ALL: [Self; 5] = [
    Self::CargoJson,
    Self::JunitXml,
    Self::VitestJson,
    Self::PlaywrightJson,
    Self::Tap,
  ];

  pub fn parse(value: &str) -> Result<Self, TestSuiteError> {
    Self::ALL
      .into_iter()
      .find(|format| format.as_str() == value)
      .ok_or_else(|| TestSuiteError::UnknownReportFormat(value.to_owned()))
  }

  pub const fn as_str(self) -> &'static str {
    match self {
      Self::CargoJson => "cargo_json",
      Self::JunitXml => "junit_xml",
      Self::VitestJson => "vitest_json",
      Self::PlaywrightJson => "playwright_json",
      Self::Tap => "tap",
    }
  }

  pub const fn output(self) -> ReportOutput {
    match self {
      Self::CargoJson | Self::Tap => ReportOutput::Printed,
      Self::JunitXml => ReportOutput::File,
      Self::VitestJson | Self::PlaywrightJson => ReportOutput::PrintedOrFile,
    }
  }
}

impl fmt::Display for ReportFormat {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

/// A report format and, when the program writes the report to a file, that file relative to the
/// suite's working folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestReport {
  format: ReportFormat,
  path: Option<SuitePath>,
}

impl TestReport {
  /// Fails when `format` writes only a file and `path` is `None`, or only prints and `path` is set.
  pub fn new(format: ReportFormat, path: Option<SuitePath>) -> Result<Self, TestSuiteError> {
    match (format.output(), &path) {
      (ReportOutput::File, None) => Err(TestSuiteError::ReportPathMissing(format)),
      (ReportOutput::Printed, Some(_)) => Err(TestSuiteError::ReportPrinted(format)),
      _ => Ok(Self { format, path }),
    }
  }

  pub const fn format(&self) -> ReportFormat {
    self.format
  }

  /// The file the report is read from; `None` when it is read from the program's output.
  pub const fn path(&self) -> Option<&SuitePath> {
    self.path.as_ref()
  }
}
