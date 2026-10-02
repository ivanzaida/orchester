//! Failures that could feed each other: steps that break in turn, and a run whose automatic
//! attempts reach their limit, wait for a person; a restart keeps the failure report and the
//! person's note of the attempt it restarts.

use orchester_core::{
  id::StepExecutionId,
  run::{MAX_AUTOMATIC_ATTEMPTS, ReviewNote, RunStatus, StepFailure, Stuck, StuckDecision},
};

use crate::{
  fixture::{Config, Logged, at, key},
  steps::{fail_checks, implemented},
};

/// Starts `step`'s pending execution, if it is pending, and fails it for `reason`.
fn fail(logged: &mut Logged, step: &str, reason: StepFailure) -> StepExecutionId {
  let execution = logged.exec(step);
  let pending = logged.run.execution(execution).map(|execution| execution.status())
    == Some(orchester_core::run::StepStatus::Pending);
  if pending {
    logged.apply(|run| run.start_step(execution, at(60)));
  }
  logged.apply(|run| run.fail_step(execution, reason, at(61)));
  execution
}

/// Starts `checks` and `review` side by side and breaks `first`.
fn break_beside(logged: &mut Logged, first: &str) {
  for step in ["checks", "review"] {
    let execution = logged.exec(step);
    logged.apply(|run| run.start_step(execution, at(50)));
  }
  let execution = logged.exec(first);
  logged.apply(|run| run.fail_step(execution, StepFailure::AgentFailed, at(51)));
}

#[test]
fn stuck_loops_steps_that_break_in_turn_wait_for_a_person() {
  let mut logged = implemented();
  break_beside(&mut logged, "checks");
  break_beside(&mut logged, "review");
  break_beside(&mut logged, "checks");
  // Checks broke twice since it last passed, though another step's restart came between.
  assert_eq!(
    logged.run.status(),
    &RunStatus::WaitingForPerson(Stuck::BrokeAgain(key("checks")))
  );
  assert_eq!(logged.run.attempts().len(), 3);
}

#[test]
fn stuck_loops_automatic_attempts_are_limited() {
  let mut logged = Logged::pending_with(&Config::feature_delivery_with_retries(50));
  logged.apply(|run| run.start(at(11)));
  logged.through_plan();
  logged.pass("implement");
  // Each round: checks returns the work to implement, which breaks once and passes after its
  // restart, so its breakages never add up. Neither limit of a step is reached.
  for _ in 0..MAX_AUTOMATIC_ATTEMPTS / 2 {
    fail_checks(&mut logged);
    fail(&mut logged, "implement", StepFailure::AgentFailed);
    logged.pass("implement");
  }
  assert_eq!(logged.run.status(), &RunStatus::Running);
  fail_checks(&mut logged);
  assert_eq!(
    logged.run.status(),
    &RunStatus::WaitingForPerson(Stuck::RetriesExhausted(key("checks")))
  );
  assert_eq!(logged.run.attempts().len(), 1 + MAX_AUTOMATIC_ATTEMPTS);
}

#[test]
fn stuck_loops_a_restart_keeps_the_report_of_the_attempt_it_restarts() {
  let mut logged = implemented();
  let failed = logged.exec("checks");
  fail_checks(&mut logged);
  // Implement, which the report reached, breaks: its restart still reports it.
  fail(&mut logged, "implement", StepFailure::AgentFailed);
  let restarted = logged.exec("implement");
  let report = logged
    .run
    .retry_report(restarted)
    .expect("the restart reports the failure");
  assert_eq!(report.failed.id(), failed);
  assert_eq!(report.attempt.get(), 1);
  logged.pass("implement");
  // Checks, which failed, breaks on its rerun: its restart still gets its own earlier result.
  fail(&mut logged, "checks", StepFailure::AgentFailed);
  let checks = logged.exec("checks");
  assert_eq!(
    logged.run.retry_report(checks).map(|report| report.failed.id()),
    Some(failed)
  );
  assert_eq!(logged.run.retry_report(logged.exec("pr")), None);
}

#[test]
fn stuck_loops_a_restart_keeps_the_persons_note() {
  let mut logged = implemented();
  for _ in 0..2 {
    fail_checks(&mut logged);
    logged.pass("implement");
  }
  fail_checks(&mut logged);
  assert_eq!(logged.run.retry_from(), Some(&key("implement")));
  let note = ReviewNote::new("Keep the session store.").expect("note");
  let back = StuckDecision::ReturnTo {
    step: key("implement"),
    note: note.clone(),
  };
  logged.apply(|run| run.decide_stuck(&back, at(70)));
  fail(&mut logged, "implement", StepFailure::AgentFailed);
  let restarted = logged.exec("implement");
  let report = logged
    .run
    .retry_report(restarted)
    .expect("the restart reports the failure");
  assert_eq!(report.note, Some(&note));
  assert_eq!(report.failed.step(), &key("checks"));
}
