//! A whole report: its tests, the errors outside any test, and their totals.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::{
  ReportFormat,
  case::{SourceLocation, TestCase, TestOutcome, capped_message},
};

/// A report read into the shared shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestReport {
  format: ReportFormat,
  tests: Vec<TestCase>,
  run_errors: Vec<RunError>,
}

impl TestReport {
  pub(super) const fn new(format: ReportFormat, tests: Vec<TestCase>, run_errors: Vec<RunError>) -> Self {
    Self {
      format,
      tests,
      run_errors,
    }
  }

  pub const fn format(&self) -> ReportFormat {
    self.format
  }

  pub fn tests(&self) -> &[TestCase] {
    &self.tests
  }

  /// Failures outside any single test: a test file that could not load, a global error, a suite whose own counts
  /// name failures its tests do not show.
  pub fn run_errors(&self) -> &[RunError] {
    &self.run_errors
  }

  pub fn failures(&self) -> impl Iterator<Item = &TestCase> {
    self.tests.iter().filter(|test| test.outcome().is_failure())
  }

  pub fn totals(&self) -> Totals {
    Totals::of(&self.tests)
  }

  /// No test failed or errored and nothing failed outside the tests.
  pub fn passed(&self) -> bool {
    self.run_errors.is_empty() && self.failures().next().is_none()
  }
}

/// A failure a report names outside any single test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunError {
  message: String,
  location: Option<SourceLocation>,
}

impl RunError {
  pub(super) fn new(message: &str, location: Option<SourceLocation>) -> Self {
    let message = capped_message(Some(message)).unwrap_or_else(|| "(no message)".to_owned());
    Self { message, location }
  }

  pub fn message(&self) -> &str {
    &self.message
  }

  pub const fn location(&self) -> Option<&SourceLocation> {
    self.location.as_ref()
  }
}

/// How many tests ended each way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
  pub passed: usize,
  pub failed: usize,
  pub skipped: usize,
  pub errored: usize,
}

impl Totals {
  pub(super) fn of(tests: &[TestCase]) -> Self {
    let mut totals = Self::default();
    for test in tests {
      match test.outcome() {
        TestOutcome::Passed => totals.passed += 1,
        TestOutcome::Failed => totals.failed += 1,
        TestOutcome::Skipped => totals.skipped += 1,
        TestOutcome::Errored => totals.errored += 1,
      }
    }
    totals
  }

  pub const fn tests(&self) -> usize {
    self.passed + self.failed + self.skipped + self.errored
  }
}

impl fmt::Display for Totals {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "{} tests ({} passed, {} failed, {} skipped, {} errored)",
      self.tests(),
      self.passed,
      self.failed,
      self.skipped,
      self.errored
    )
  }
}
