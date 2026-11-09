//! The part a step after a Test step reads about it (`docs/architecture/agent-testing.md`, "The Test step"): the
//! result the Test step stored with the run, and that single checks go through `run_tests(filter)`, so a Review or a
//! Verify step does not run every suite again.

use std::{fmt::Write as _, num::NonZeroU32};

use super::composition::push_block;
use crate::resource::Step;

/// What the agent is told before the stored result.
const LEAD: &str = "Orchester ran the project's test suites on this work in the Test step below and kept its result \
                    with the run. Do not run every suite again: to check a single test, call run_tests with the \
                    suite and a filter, and read one test's whole output with test_output.";

/// The result a passed Test step the prompt's step runs after stored with the run.
#[derive(Clone, Copy, Debug)]
pub struct StoredTestResult<'a> {
  pub step: &'a Step,
  pub attempt: NonZeroU32,
  pub text: &'a str,
}

/// Appends the test results part for `results`, if any.
pub(super) fn push(text: &mut String, results: &[StoredTestResult<'_>]) {
  if results.is_empty() {
    return;
  }
  text.push_str("\n# Test results\n");
  push_block(text, LEAD);
  for result in results {
    // Writing to a String cannot fail.
    let _ = writeln!(
      text,
      "\n## {} (`{}`, attempt {})",
      result.step.name.as_str(),
      result.step.key,
      result.attempt
    );
    push_block(text, result.text);
  }
}
