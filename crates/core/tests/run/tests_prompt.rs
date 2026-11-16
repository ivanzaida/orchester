//! The part a step after a passed Test step reads: the result that step stored with the run, under its source, and
//! that single checks go through `run_tests` with a filter instead of running every suite again.

use std::num::NonZeroU32;

use orchester_core::{
  resource::{InjectionEvent, Step, StepKind},
  run::{PromptInput, StoredTestResult},
};

use crate::{
  fixture::{Config, key, task},
  prompt::{Prompt, builder, step_with},
};

const STORED: &str = "The Test step passed: 1 suites ran, layer by layer.\n\n- unit · `unit`: 2 tests (2 passed, 0 \
                      failed, 0 skipped, 0 errored)";

fn tests_step() -> Step {
  let mut step = Config::feature_delivery()
    .pipeline_version
    .body()
    .step(&key("review"))
    .expect("step")
    .clone();
  step.kind = StepKind::Test;
  step
}

fn assembled(prompt: &Prompt, tests: &[StoredTestResult<'_>], event: InjectionEvent) -> String {
  PromptInput {
    tests,
    ..prompt.input(&[], &[], true)
  }
  .assemble(event)
  .expect("no variables")
  .text
}

#[test]
fn tests_prompt_a_later_step_reads_the_stored_result_and_checks_single_tests() {
  let prompt = Prompt::new(builder("Role."), step_with("Review it."), task("Brief.", &[]));
  let step = tests_step();
  let stored = [StoredTestResult {
    step: &step,
    attempt: NonZeroU32::MIN,
    text: STORED,
  }];
  let text = assembled(&prompt, &stored, InjectionEvent::Start);
  let part = text.find("\n# Test results\n").expect("the part");
  assert!(part > text.find("# Task ").expect("the task part"), "{text}");
  assert!(
    part < text.find("# When you finish").expect("the result instructions"),
    "{text}"
  );
  assert!(
    text.contains("Do not run every suite again: to check a single test, call run_tests with the suite and a filter"),
    "{text}"
  );
  assert!(text.contains("## review (`review`, attempt 1)"), "{text}");
  assert!(text.contains("- unit · `unit`: 2 tests (2 passed"), "{text}");
  // A resumed session read it when it started; a step after no Test step reads nothing of the kind.
  assert!(!assembled(&prompt, &stored, InjectionEvent::Resume).contains("# Test results"));
  assert!(!assembled(&prompt, &[], InjectionEvent::Start).contains("# Test results"));
}
