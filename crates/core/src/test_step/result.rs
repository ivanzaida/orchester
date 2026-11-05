//! What a Test step found, and how it reads for the agents after it: the result the step leaves for the steps after it
//! when it passes ([`TestStepResult::summary`]), and the failed tests alone when it fails
//! ([`TestStepResult::failure_report`]) — each with its name, message, file:line, excerpt and artifacts, never the
//! programs' raw output, which stays with the run for `test_output`.

use std::fmt::Write as _;

use super::outcome::SuiteOutcome;
use crate::{
  resource::{Slug, TestLayer},
  test_report::MAX_FAILURES_SHOWN,
};

/// The most characters of a failed test's message the report shows; its excerpt has the start of the rest.
const MESSAGE_CHARS: usize = 400;

/// How a Test step ran: each layer it reached with its suites' outcomes, the layers it did not reach because one
/// before failed, and the suites the run's changes left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestStepResult {
  pub reached: Vec<(TestLayer, Vec<SuiteOutcome>)>,
  pub not_reached: Vec<(TestLayer, Vec<Slug>)>,
  pub left_out: Vec<Slug>,
}

impl TestStepResult {
  /// Whether every suite it ran passed.
  pub fn passed(&self) -> bool {
    self
      .reached
      .iter()
      .all(|(_, suites)| suites.iter().all(SuiteOutcome::passed))
  }

  /// The layer it stopped at, if one failed.
  pub fn failed_layer(&self) -> Option<TestLayer> {
    self
      .reached
      .iter()
      .find(|(_, suites)| !suites.iter().all(SuiteOutcome::passed))
      .map(|(layer, _)| *layer)
  }

  /// What the steps after it read when it passed: the suites it ran with their totals, the flaky tests, and the
  /// suites it left out.
  pub fn summary(&self) -> String {
    let suites: Vec<&SuiteOutcome> = self.suites().collect();
    if suites.is_empty() {
      let mut text = "The Test step ran no suite: the run changed no path a service of a suite owns, and no suite is \
                      marked always."
        .to_owned();
      self.push_left_out(&mut text);
      return text;
    }
    let mut text = format!("The Test step passed: {} suites ran, layer by layer.\n", suites.len());
    for suite in &suites {
      // Writing to a String cannot fail.
      let _ = write!(text, "\n- {} · `{}`", suite.layer.as_str(), suite.suite.as_str());
      if let Some(totals) = suite.totals() {
        let _ = write!(text, ": {totals}");
      }
      if let Some((run, stored)) = &suite.kept {
        let _ = write!(text, " — test run {}, kept in {stored}", run.id);
      }
    }
    text.push('\n');
    self.push_flaky(&mut text);
    self.push_left_out(&mut text);
    text
  }

  /// What the writing step reads when it failed: the layer it stopped at, and only what failed there — each failed
  /// test and each suite that failed without one.
  pub fn failure_report(&self) -> String {
    let layer = self.failed_layer().map_or("a", TestLayer::as_str);
    let failing: Vec<&SuiteOutcome> = self.suites().filter(|suite| !suite.passed()).collect();
    let tests: usize = failing.iter().map(|suite| suite.failed.len()).sum();
    let mut text = format!(
      "The Test step failed at the {layer} layer: {tests} tests failed in {} suites. Only what failed is listed.\n",
      failing.len()
    );
    let mut shown = 0;
    for suite in &failing {
      // Writing to a String cannot fail.
      let _ = write!(text, "\n## {} · `{}`", suite.layer.as_str(), suite.suite.as_str());
      if let Some((run, stored)) = &suite.kept {
        let _ = write!(text, " (test run {}, kept in {stored})", run.id);
      }
      text.push('\n');
      if let Some(problem) = &suite.problem {
        let _ = writeln!(text, "\nThe suite did not pass: {problem}.");
      }
      for test in &suite.failed {
        if shown == MAX_FAILURES_SHOWN {
          break;
        }
        shown += 1;
        push_test(&mut text, test);
      }
    }
    if tests > shown {
      let _ = writeln!(text, "\n{} more failed tests are not listed.", tests - shown);
    }
    self.push_flaky(&mut text);
    let skipped: Vec<String> = self
      .not_reached
      .iter()
      .map(|(layer, suites)| format!("{} ({})", layer.as_str(), quoted(suites)))
      .collect();
    if !skipped.is_empty() {
      let _ = writeln!(text, "\nNot run, after the failing layer: {}.", skipped.join(", "));
    }
    text.push_str(
      "\nRun one test again with run_tests(suite, filter), and read one test's whole output with test_output(run, \
       test).\n",
    );
    text
  }

  fn suites(&self) -> impl Iterator<Item = &SuiteOutcome> {
    self.reached.iter().flat_map(|(_, suites)| suites.iter())
  }

  fn push_flaky(&self, text: &mut String) {
    let flaky: Vec<String> = self
      .suites()
      .flat_map(|suite| {
        suite
          .flaky
          .iter()
          .map(move |test| format!("`{test}` in `{}`", suite.suite.as_str()))
      })
      .collect();
    if !flaky.is_empty() {
      // Writing to a String cannot fail.
      let _ = writeln!(
        text,
        "\nFlaky, so they do not fail the step: they failed, then passed when run again: {}.",
        flaky.join(", ")
      );
    }
  }

  fn push_left_out(&self, text: &mut String) {
    if !self.left_out.is_empty() {
      // Writing to a String cannot fail.
      let _ = writeln!(
        text,
        "\nLeft out, since the run changed no path their services own: {}.",
        quoted(&self.left_out)
      );
    }
  }
}

fn push_test(text: &mut String, test: &super::outcome::FailedTest) {
  // Writing to a String cannot fail.
  let _ = write!(text, "\n- `{}`: {}", test.name, test.outcome);
  if let Some(location) = &test.location {
    let _ = write!(text, " at {location}");
  }
  text.push('\n');
  if let Some(message) = &test.message {
    let _ = writeln!(text, "  message: {}", short(message.lines().next().unwrap_or_default()));
  }
  if let Some(excerpt) = &test.excerpt {
    text.push_str("  excerpt:\n");
    for line in excerpt.lines() {
      let _ = writeln!(text, "    {line}");
    }
  }
  if !test.artifacts.is_empty() {
    let _ = writeln!(text, "  artifacts: {}", test.artifacts.join(", "));
  }
}

fn quoted(slugs: &[Slug]) -> String {
  slugs
    .iter()
    .map(|slug| format!("`{}`", slug.as_str()))
    .collect::<Vec<_>>()
    .join(", ")
}

/// The start of `message`, its first [`MESSAGE_CHARS`] characters.
fn short(message: &str) -> String {
  match message.char_indices().nth(MESSAGE_CHARS) {
    Some((end, _)) => format!("{}…", &message[..end]),
    None => message.to_owned(),
  }
}
