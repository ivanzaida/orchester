//! Running the step graph: which executions are ready, how many start with the free agent slots,
//! and how a failure among parallel steps ends and routes the attempt.

use orchester_core::run::{
  AttemptReason, RunFailure, RunStatus, StepFailure, StepOutcome, StepOutput, StepResult, StepStatus,
};

use crate::fixture::{Logged, at, key};

fn names(logged: &Logged, executions: &[orchester_core::id::StepExecutionId]) -> Vec<String> {
  executions
    .iter()
    .map(|id| logged.run.execution(*id).expect("execution").step().to_string())
    .collect()
}

/// Passes everything up to and including `implement`: `checks` and `review` are ready together.
fn implemented() -> Logged {
  let mut logged = Logged::started();
  logged.through_plan();
  logged.pass("implement");
  logged
}

#[test]
fn schedule_ready_set_holds_steps_whose_dependencies_passed() {
  let mut logged = Logged::started();
  assert_eq!(names(&logged, &logged.run.ready_executions()), ["research"]);
  logged.pass("research");
  assert_eq!(names(&logged, &logged.run.ready_executions()), ["plan"]);
  let logged = implemented();
  assert_eq!(names(&logged, &logged.run.ready_executions()), ["checks", "review"]);
  assert!(logged.run.running_executions().is_empty());
}

#[test]
fn schedule_slot_bound_starts_at_most_the_free_slots_in_pipeline_order() {
  let mut logged = implemented();
  assert!(logged.run.executions_to_start(0).is_empty());
  assert_eq!(names(&logged, &logged.run.executions_to_start(1)), ["checks"]);
  assert_eq!(names(&logged, &logged.run.executions_to_start(4)), ["checks", "review"]);
  let checks = logged.exec("checks");
  logged.apply(|run| run.start_step(checks, at(30)));
  assert_eq!(names(&logged, &logged.run.executions_to_start(4)), ["review"]);
  assert_eq!(names(&logged, &logged.run.running_executions()), ["checks"]);
  // A run waiting to pause starts nothing new.
  logged.apply(|run| run.request_pause(at(31)));
  assert!(logged.run.executions_to_start(4).is_empty());
}

#[test]
fn schedule_parallel_steps_pass_independently_and_join() {
  let mut logged = implemented();
  let (checks, review) = (logged.exec("checks"), logged.exec("review"));
  logged.apply(|run| run.start_step(checks, at(30)));
  logged.apply(|run| run.start_step(review, at(30)));
  logged.apply(|run| run.complete_step(review, at(31)));
  // `pr` joins both: it waits for `checks`.
  assert!(logged.run.ready_executions().is_empty());
  logged.apply(|run| run.complete_step(checks, at(32)));
  assert_eq!(names(&logged, &logged.run.ready_executions()), ["pr"]);
}

#[test]
fn schedule_failure_skips_parallel_steps_and_routes_with_its_reason() {
  let mut logged = implemented();
  let (checks, review) = (logged.exec("checks"), logged.exec("review"));
  logged.apply(|run| run.start_step(checks, at(30)));
  logged.apply(|run| run.start_step(review, at(30)));
  logged.apply(|run| run.fail_step(checks, StepFailure::MergeConflict, at(31)));
  let routed = &logged.run.attempts()[0];
  let failed = routed.execution(checks).expect("checks");
  assert_eq!(failed.status(), StepStatus::Failed);
  assert_eq!(failed.failure(), Some(StepFailure::MergeConflict));
  assert_eq!(
    routed.execution(review).map(|review| review.status()),
    Some(StepStatus::Skipped)
  );
  assert_eq!(
    logged.run.current_attempt().map(|attempt| attempt.reason().clone()),
    Some(AttemptReason::Retry {
      failed_step: key("checks"),
      return_to: key("implement"),
    })
  );
  assert_eq!(names(&logged, &logged.run.ready_executions()), ["implement"]);
  // The reason survives a replay of the log.
  let replayed = orchester_core::run::Run::replay(&logged.log).expect("valid log");
  assert_eq!(replayed, logged.run);
}

#[test]
fn schedule_retry_limit_fails_the_run_once_the_route_was_taken_max_times() {
  let mut logged = implemented();
  for attempt in 0..3 {
    if attempt > 0 {
      logged.pass("implement");
    }
    let checks = logged.exec("checks");
    logged.apply(|run| run.start_step(checks, at(40)));
    logged.apply(|run| run.fail_step(checks, StepFailure::ReportedFailed, at(41)));
  }
  assert_eq!(logged.run.retries_taken(&key("checks")), 2);
  assert_eq!(
    logged.run.status(),
    &RunStatus::Failed(RunFailure::RetriesExhausted(key("checks")))
  );
  assert_eq!(logged.run.attempts().len(), 3);
}

#[test]
fn schedule_step_result_reads_the_structured_final_message() {
  assert_eq!(
    StepResult::parse(r#" {"outcome":"failed","summary":"Tests fail."} "#),
    Some(StepResult {
      outcome: StepOutcome::Failed,
      summary: "Tests fail.".into(),
    })
  );
  assert_eq!(StepResult::parse("Done."), None);
  assert_eq!(StepResult::parse(r#"{"outcome":"done","summary":"x","extra":1}"#), None);
  let logged = implemented();
  let checks = logged.exec("checks");
  let structured = StepOutput::from_final_message(
    logged.run.id(),
    checks,
    key("checks"),
    Some(r#"{"outcome":"done","summary":"All green."}"#),
    at(50),
  );
  assert_eq!(
    (structured.outcome, structured.text.as_str()),
    (Some(StepOutcome::Done), "All green.")
  );
  let plain = StepOutput::from_final_message(logged.run.id(), checks, key("checks"), Some(" Done.\n"), at(50));
  assert_eq!((plain.outcome, plain.text.as_str()), (None, "Done."));
}
