//! What follows a failure: a verdict takes the failure route, a breakage restarts its step once, and
//! with no route left the run waits for a person, who allows one more retry, returns the work to an
//! agent step with a note, or stops the run (user decisions 2026-10-02).

use orchester_core::{
  id::{RunId, StepExecutionId},
  run::{
    AttemptReason, AttemptStatus, Run, RunCounts, RunError, RunFailure, RunStatus, StepFailure, StepOutcome,
    StepOutput, StepStatus, Stuck, StuckDecision, TaskPhase,
  },
};

use crate::{
  fixture::{Logged, at, key, stored, task},
  steps::{fail_checks, implemented},
};

fn note(text: &str) -> orchester_core::run::ReviewNote {
  orchester_core::run::ReviewNote::new(text).expect("a note")
}

/// `checks` fails its verdict until its two retries are used up: the run waits for a person.
fn exhausted() -> Logged {
  let mut logged = implemented();
  fail_checks(&mut logged);
  logged.pass("implement");
  fail_checks(&mut logged);
  logged.pass("implement");
  fail_checks(&mut logged);
  logged
}

#[test]
fn stuck_only_verdicts_follow_the_failure_route() {
  for verdict in [StepFailure::ReportedFailed, StepFailure::CommandFailed] {
    assert!(verdict.is_verdict(), "{verdict:?}");
  }
  for breakage in [
    StepFailure::AgentFailed,
    StepFailure::NotStarted,
    StepFailure::MergeConflict,
    StepFailure::CheckpointFailed,
  ] {
    assert!(!breakage.is_verdict(), "{breakage:?}");
  }
  let mut logged = implemented();
  fail_checks(&mut logged);
  assert!(matches!(
    logged.run.current_attempt().map(|attempt| attempt.reason()),
    Some(AttemptReason::Retry { .. })
  ));
}

#[test]
fn stuck_a_breakage_restarts_its_step_once_then_waits_for_a_person() {
  let mut logged = implemented();
  let checks = logged.exec("checks");
  logged.apply(|run| run.start_step(checks, at(40)));
  logged.apply(|run| run.fail_step(checks, StepFailure::AgentFailed, at(41)));
  let restart = logged.run.current_attempt().expect("a restart");
  assert_eq!(restart.reason(), &AttemptReason::Restart { step: key("checks") });
  let steps: Vec<_> = restart.executions().iter().map(|e| e.step().as_str()).collect();
  // The route's target keeps its result: a breakage never returns to an upstream step.
  assert_eq!(steps, ["checks", "review", "pr"]);
  assert_eq!(logged.run.retries_taken(&key("checks")), 0);
  let again = logged.exec("checks");
  assert_ne!(again, checks);
  logged.apply(|run| run.start_step(again, at(42)));
  logged.apply(|run| run.fail_step(again, StepFailure::NotStarted, at(43)));
  assert_eq!(
    logged.run.status(),
    &RunStatus::WaitingForPerson(Stuck::BrokeAgain(key("checks")))
  );
  assert_eq!(logged.run.attempts().len(), 2);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run.clone()));
  assert_eq!(Run::restore(stored(&logged.run)), Ok(logged.run));
}

#[test]
fn stuck_a_step_without_a_route_restarts_after_a_breakage_and_fails_on_a_verdict() {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.fail_step(research, StepFailure::AgentFailed, at(13)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
  let again = logged.exec("research");
  logged.apply(|run| run.start_step(again, at(14)));
  logged.apply(|run| run.fail_step(again, StepFailure::ReportedFailed, at(15)));
  assert_eq!(
    logged.run.status(),
    &RunStatus::Failed(RunFailure::StepFailed(key("research")))
  );
}

#[test]
fn stuck_exhausted_retries_wait_and_count_as_needing_you() {
  let logged = exhausted();
  let stuck = RunStatus::WaitingForPerson(Stuck::RetriesExhausted(key("checks")));
  assert_eq!(logged.run.status(), &stuck);
  assert!(!logged.run.status().is_terminal());
  assert!(logged.run.executions_to_start(4).is_empty());
  let task = task("Brief.", &[]);
  let phase = TaskPhase::of(&task, Some(&logged.run), None);
  assert_eq!(phase, TaskPhase::Stuck);
  assert!(phase.needs_you() && !phase.is_active());
  assert_eq!(phase.without_runner(), TaskPhase::Stuck);
  assert_eq!(RunCounts::of([phase], false).needs_you, 1);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run.clone()));
  assert_eq!(Run::restore(stored(&logged.run)), Ok(logged.run));
}

#[test]
fn stuck_one_more_retry_takes_the_same_route_and_waits_again_after_it() {
  let mut logged = exhausted();
  logged.apply(|run| run.decide_stuck(&StuckDecision::Retry, at(50)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
  let retry = logged.run.current_attempt().expect("a retry");
  assert_eq!(
    retry.reason(),
    &AttemptReason::Retry {
      failed_step: key("checks"),
      return_to: key("implement"),
    }
  );
  assert_eq!(logged.run.attempts()[2].status(), AttemptStatus::RoutedBack);
  assert_eq!(logged.run.retries_taken(&key("checks")), 3);
  // The retry the person allowed is not one the route took by itself.
  assert_eq!(logged.run.automatic_retries_taken(&key("checks")), 2);
  logged.pass("implement");
  fail_checks(&mut logged);
  assert_eq!(
    logged.run.status(),
    &RunStatus::WaitingForPerson(Stuck::RetriesExhausted(key("checks")))
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn stuck_a_broken_step_is_allowed_one_more_restart() {
  let mut logged = implemented();
  for _ in 0..2 {
    let checks = logged.exec("checks");
    logged.apply(|run| run.start_step(checks, at(40)));
    logged.apply(|run| run.fail_step(checks, StepFailure::AgentFailed, at(41)));
  }
  logged.apply(|run| run.decide_stuck(&StuckDecision::Retry, at(50)));
  assert_eq!(
    logged.run.current_attempt().map(|attempt| attempt.reason()),
    Some(&AttemptReason::Restart { step: key("checks") })
  );
  assert_eq!(logged.run.status(), &RunStatus::Running);
}

#[test]
fn stuck_work_returns_to_a_chosen_agent_step_with_the_persons_note() {
  let mut logged = exhausted();
  assert_eq!(
    logged.run.return_targets(),
    [key("research"), key("plan"), key("implement"), key("checks")]
  );
  let refused = StuckDecision::ReturnTo {
    step: key("review"),
    note: note("Fix it."),
  };
  assert_eq!(
    logged.run.decide_stuck(&refused, at(50)),
    Err(RunError::NotAReturnTarget(key("review")))
  );
  let back = StuckDecision::ReturnTo {
    step: key("plan"),
    note: note("Use the existing session store."),
  };
  logged.apply(|run| run.decide_stuck(&back, at(51)));
  let attempt = logged.run.current_attempt().expect("the returned attempt");
  assert_eq!(
    attempt.reason(),
    &AttemptReason::Returned {
      failed_step: key("checks"),
      return_to: key("plan"),
      note: note("Use the existing session store."),
    }
  );
  let steps: Vec<_> = attempt.executions().iter().map(|e| e.step().as_str()).collect();
  assert_eq!(steps, ["plan", "implement", "checks", "review", "pr"]);
  let plan = logged.exec("plan");
  let report = logged.run.retry_report(plan).expect("the plan step learns why");
  assert_eq!(report.failed.step(), &key("checks"));
  assert_eq!(report.attempt.get(), 3);
  assert_eq!(report.note, Some(&note("Use the existing session store.")));
  let checks = logged.exec("checks");
  let own = logged.run.retry_report(checks).expect("checks gets its own result");
  assert_eq!(own.note, None);
  assert_eq!(logged.run.retry_report(logged.exec("implement")), None);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run.clone()));
  assert_eq!(Run::restore(stored(&logged.run)), Ok(logged.run));
}

#[test]
fn stuck_stopping_fails_the_run_as_before() {
  let mut logged = exhausted();
  logged.apply(|run| run.decide_stuck(&StuckDecision::Stop, at(50)));
  assert_eq!(
    logged.run.status(),
    &RunStatus::Failed(RunFailure::RetriesExhausted(key("checks")))
  );
  assert_eq!(logged.run.attempts()[2].status(), AttemptStatus::Failed);
  assert!(logged.run.decide_stuck(&StuckDecision::Retry, at(51)).is_err());
  // A failed run is still recovered by a person's message.
  assert!(logged.run.recovery_step(None).is_some());
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn stuck_decisions_need_a_run_that_waits() {
  let mut logged = implemented();
  assert!(matches!(
    logged.run.decide_stuck(&StuckDecision::Retry, at(50)),
    Err(RunError::InvalidRunTransition { .. })
  ));
  assert!(logged.run.return_targets().is_empty());
}

#[test]
fn stuck_a_retry_reports_the_failure_to_its_target_and_the_failed_step() {
  let mut logged = implemented();
  let failed = logged.exec("checks");
  fail_checks(&mut logged);
  let implement = logged.exec("implement");
  let report = logged.run.retry_report(implement).expect("implement learns why");
  assert_eq!(report.failed.id(), failed);
  assert_eq!(report.failed.status(), StepStatus::Failed);
  assert_eq!(report.attempt.get(), 1);
  assert_eq!(report.note, None);
  let checks = logged.exec("checks");
  assert_eq!(
    logged.run.retry_report(checks).map(|report| report.failed.id()),
    Some(failed)
  );
  assert_eq!(logged.run.retry_report(logged.exec("review")), None);
  // A restart reports nothing: nothing judged the work.
  let mut broken = Logged::started();
  let research = broken.exec("research");
  broken.apply(|run| run.start_step(research, at(12)));
  broken.apply(|run| run.fail_step(research, StepFailure::AgentFailed, at(13)));
  assert_eq!(broken.run.retry_report(broken.exec("research")), None);
}

#[test]
fn stuck_a_failed_or_question_result_wins_over_a_failed_turn() {
  let output = |outcome: Option<StepOutcome>| StepOutput {
    run: RunId::generate(),
    execution: StepExecutionId::generate(),
    step: key("review"),
    outcome,
    text: "Two blocking findings.".into(),
    created_at: at(1),
  };
  // Run 19: Review returned `failed`, then its turn failed when the permission prompt closed.
  assert_eq!(
    output(Some(StepOutcome::Failed)).failure(false),
    Some(StepFailure::ReportedFailed)
  );
  // Nothing outweighs a failed turn but a `failed` verdict: a step never passes after its permission
  // prompt failed.
  assert_eq!(
    output(Some(StepOutcome::Done)).failure(false),
    Some(StepFailure::AgentFailed)
  );
  assert_eq!(output(Some(StepOutcome::Done)).failure(true), None);
  assert_eq!(output(Some(StepOutcome::Question)).failure(true), None);
  // Run 26: Verify asked a question, then its turn failed when the permission prompt closed.
  assert_eq!(output(Some(StepOutcome::Question)).failure(false), None);
  assert_eq!(output(None).failure(false), Some(StepFailure::AgentFailed));
  assert_eq!(output(None).failure(true), None);
  assert_eq!(
    output(Some(StepOutcome::Failed)).failure(true),
    Some(StepFailure::ReportedFailed)
  );
}
