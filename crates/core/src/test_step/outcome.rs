//! How one suite of a Test step came out: its first run, the tests that failed in it run once more, and what is left
//! failing. A failed test that passes when run again is flaky and does not fail the suite; a quarantined test never
//! fails it either, but there is no quarantine list yet (ORC-259), so none is.

use crate::{
  resource::{Slug, TestFilter, TestLayer, TestSuite},
  test_report::{SuiteRun, SuiteRunEnd, TestCase, Totals, test_name},
};

/// The most failed tests of one suite run again one by one; with more, the suite runs once more whole.
pub const MAX_SINGLE_RERUNS: usize = 10;

/// A test that failed in its suite's run and when run again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedTest {
  /// Its suites and its own name, as `run_tests` names it ([`test_name`]).
  pub name: String,
  pub outcome: &'static str,
  pub message: Option<String>,
  /// `file:line`, or the file alone.
  pub location: Option<String>,
  pub excerpt: Option<String>,
  /// The stored copies of what its report attached to it.
  pub artifacts: Vec<String>,
}

/// One suite's part of a Test step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiteOutcome {
  pub suite: Slug,
  pub layer: TestLayer,
  /// Its first run, kept with the run as `run_tests` keeps one, and where its result is; `None` when it did not run.
  pub kept: Option<(SuiteRun, String)>,
  /// The tests still failing after they ran again.
  pub failed: Vec<FailedTest>,
  /// The tests that failed, then passed when run again.
  pub flaky: Vec<String>,
  /// Why the suite fails besides its failed tests: it did not run, its report cannot be read, it timed out, the
  /// report names errors outside the tests, or the program's exit code says it failed.
  pub problem: Option<String>,
}

impl SuiteOutcome {
  /// A suite that could not run, for `why`.
  pub fn not_run(suite: Slug, layer: TestLayer, why: String) -> Self {
    Self {
      suite,
      layer,
      kept: None,
      failed: Vec::new(),
      flaky: Vec::new(),
      problem: Some(why),
    }
  }

  /// A suite whose first run is `first`, kept at `stored`, after its failed tests ran again in `reruns`: a failed
  /// test that one of them shows passed is flaky, every other one still fails.
  pub fn settle(suite: Slug, layer: TestLayer, (first, stored): (SuiteRun, String), reruns: &[SuiteRun]) -> Self {
    let mut outcome = Self {
      suite,
      layer,
      kept: None,
      failed: Vec::new(),
      flaky: Vec::new(),
      problem: problem(&first),
    };
    if let Some(report) = first.report() {
      for case in report.failures() {
        let name = test_name(case);
        if passed_again(&name, reruns) {
          if !outcome.flaky.contains(&name) {
            outcome.flaky.push(name);
          }
        } else {
          outcome.failed.push(failed(&first, case, name));
        }
      }
    }
    outcome.kept = Some((first, stored));
    outcome
  }

  /// Whether the suite passed: nothing is left failing and nothing else went wrong.
  pub fn passed(&self) -> bool {
    self.failed.is_empty() && self.problem.is_none()
  }

  /// The totals of its first run, once its report was read.
  pub fn totals(&self) -> Option<Totals> {
    self.kept.as_ref()?.0.report().map(|report| report.totals())
  }
}

/// The filters the tests that failed in `first` run again with: one per failed test, by its own name, when `suite`
/// takes a filter and no more than [`MAX_SINGLE_RERUNS`] failed; else the whole suite once (`None`). Nothing runs again
/// for a run without failed tests, or with errors outside them, which a test passing again does not undo.
pub fn rerun_filters(suite: &TestSuite, first: &SuiteRun) -> Vec<Option<TestFilter>> {
  let Some(report) = first.report() else {
    return Vec::new();
  };
  if !report.run_errors().is_empty() {
    return Vec::new();
  }
  let mut names: Vec<&str> = Vec::new();
  for case in report.failures() {
    if !names.contains(&case.name()) {
      names.push(case.name());
    }
  }
  if names.is_empty() {
    return Vec::new();
  }
  let filters: Option<Vec<TestFilter>> = names.iter().map(|name| TestFilter::new(name).ok()).collect();
  match filters {
    Some(filters) if suite.command().accepts_filter() && filters.len() <= MAX_SINGLE_RERUNS => {
      filters.into_iter().map(Some).collect()
    }
    _ => vec![None],
  }
}

/// Whether one of `reruns` shows the test `name` passed and none shows it failed.
fn passed_again(name: &str, reruns: &[SuiteRun]) -> bool {
  let outcomes: Vec<bool> = reruns
    .iter()
    .filter_map(SuiteRun::report)
    .flat_map(|report| report.tests().iter())
    .filter(|case| test_name(case) == name)
    .map(|case| case.outcome().is_failure())
    .collect();
  !outcomes.is_empty() && !outcomes.contains(&true)
}

fn failed(run: &SuiteRun, case: &TestCase, name: String) -> FailedTest {
  FailedTest {
    artifacts: run.artifacts_of(&name).map(str::to_owned).collect(),
    name,
    outcome: case.outcome().as_str(),
    message: case.message().map(str::to_owned),
    location: case.location().map(|location| match location.line {
      Some(line) => format!("{}:{line}", location.file),
      None => location.file.clone(),
    }),
    excerpt: case.excerpt().map(str::to_owned),
  }
}

/// What fails `run` besides its failed tests, if anything.
fn problem(run: &SuiteRun) -> Option<String> {
  match &run.end {
    SuiteRunEnd::Unreadable { error, .. } => Some(format!(
      "its report cannot be read ({error}); exit code {}",
      exit_code(run.exit_code)
    )),
    SuiteRunEnd::TimedOut { timeout_secs } => Some(format!(
      "it did not finish within {timeout_secs} seconds and was stopped with everything it started"
    )),
    SuiteRunEnd::Read { report } if !report.run_errors().is_empty() => {
      let errors: Vec<String> = report
        .run_errors()
        .iter()
        .map(|error| match error.location() {
          Some(location) => format!("{} ({})", error.message(), location.file),
          None => error.message().to_owned(),
        })
        .collect();
      Some(format!(
        "its report names errors outside the tests: {}",
        errors.join("; ")
      ))
    }
    SuiteRunEnd::Read { report } if report.passed() && run.exit_code != Some(0) => Some(format!(
      "the report shows no failure, but the program exited with {}",
      exit_code(run.exit_code)
    )),
    SuiteRunEnd::Read { .. } => None,
  }
}

fn exit_code(code: Option<i32>) -> String {
  code.map_or_else(
    || "none (stopped or ended by a signal)".to_owned(),
    |code| code.to_string(),
  )
}
