//! The Test step (`docs/architecture/agent-testing.md`, "The Test step"): a pipeline step that runs the project's
//! published test suites in layers — lint, then unit, then e2e — and stops at the first layer that fails. Only the
//! suites whose services own a path the run changed run, plus those marked always ([`TestPlan`]). Each test that
//! fails runs once more ([`rerun_filters`]); passing then marks it flaky, and it does not fail the step
//! ([`SuiteOutcome::settle`]). The runner runs the suites as `run_tests` does and keeps each run with the run; the
//! step's result ([`TestStepResult`]) is its output: what later steps read when it passed, only the failed tests when
//! it failed, which its failure route returns to the writing step.

mod outcome;
mod plan;
mod result;

pub use self::{
  outcome::{FailedTest, MAX_SINGLE_RERUNS, SuiteOutcome, rerun_filters},
  plan::{Chosen, PlannedLayer, PlannedSuite, TestPlan},
  result::TestStepResult,
};
use crate::run::{StepAt, StepFailure, StepOutcome, StepOutput};

impl TestStepResult {
  /// Why the step fails; `None` when it passes. Failed tests are a verdict on the work, so the step's failure route
  /// returns it to the writing step.
  pub fn failure(&self) -> Option<StepFailure> {
    (!self.passed()).then_some(StepFailure::CommandFailed)
  }

  /// What the step leaves for the steps after it: its summary when it passed, the failed tests when it failed.
  pub fn output(&self, at: StepAt) -> StepOutput {
    let (outcome, text) = if self.passed() {
      (StepOutcome::Done, self.summary())
    } else {
      (StepOutcome::Failed, self.failure_report())
    };
    StepOutput {
      run: at.run,
      execution: at.execution,
      step: at.step,
      outcome: Some(outcome),
      text,
      unverified: Vec::new(),
      created_at: at.now,
    }
  }
}
