use orchester_core::run::{
  AttemptReason, AttemptStatus, PlanDecision, RunError, RunEventKind, RunFailure, RunStatus, StepFailure, StepStatus,
  WaitReason,
};

use crate::fixture::{Logged, at, key};

fn status(logged: &Logged, step: &str) -> StepStatus {
  logged.run.latest_execution(&key(step)).expect("execution").status()
}

/// Passes everything up to and including `implement`.
fn implemented() -> Logged {
  let mut logged = Logged::started();
  logged.through_plan();
  logged.pass("implement");
  logged
}

/// Starts `checks` and `review` in parallel, then fails `checks`.
fn fail_checks(logged: &mut Logged) {
  let checks = logged.exec("checks");
  let review = logged.exec("review");
  logged.apply(|run| run.start_step(checks, at(40)));
  logged.apply(|run| run.start_step(review, at(40)));
  logged.apply(|run| run.fail_step(checks, StepFailure::AgentFailed, at(41)));
}

#[test]
fn run_steps_start_only_after_their_dependencies_passed() {
  let mut logged = Logged::started();
  let plan = logged.exec("plan");
  assert_eq!(
    logged.run.start_step(plan, at(12)),
    Err(RunError::DependenciesNotPassed(key("plan")))
  );
  logged.pass("research");
  logged.apply(|run| run.start_step(plan, at(13)));
  assert_eq!(status(&logged, "plan"), StepStatus::Running);
  assert!(matches!(
    logged.run.start_step(plan, at(14)),
    Err(RunError::InvalidStepTransition {
      status: StepStatus::Running,
      ..
    })
  ));
}

#[test]
fn run_steps_wait_for_approval_or_message_and_resume() {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  assert!(logged.run.wait_step(research, WaitReason::Message, at(12)).is_err());
  logged.apply(|run| run.start_step(research, at(12)));
  for reason in [WaitReason::Approval, WaitReason::Message] {
    logged.apply(|run| run.wait_step(research, reason, at(13)));
    assert_eq!(status(&logged, "research"), StepStatus::Waiting(reason));
    assert!(logged.run.complete_step(research, at(14)).is_err());
    logged.apply(|run| run.resume_step(research, at(14)));
    assert_eq!(status(&logged, "research"), StepStatus::Running);
  }
  assert!(logged.run.resume_step(research, at(15)).is_err());
}

#[test]
fn run_steps_plan_waits_are_reserved_for_gated_steps() {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  assert_eq!(
    logged.run.wait_step(research, WaitReason::Plan, at(13)),
    Err(RunError::NotGated(key("research")))
  );
  assert!(matches!(
    logged.run.decide_plan(research, PlanDecision::Approved, at(13)),
    Err(RunError::InvalidStepTransition { .. })
  ));
}

#[test]
fn run_steps_gated_step_waits_for_a_plan_decision() {
  let mut logged = Logged::started();
  logged.pass("research");
  let plan = logged.exec("plan");
  logged.apply(|run| run.start_step(plan, at(20)));
  logged.apply(|run| run.complete_step(plan, at(21)));
  assert_eq!(status(&logged, "plan"), StepStatus::Waiting(WaitReason::Plan));
  let implement = logged.exec("implement");
  assert_eq!(
    logged.run.start_step(implement, at(22)),
    Err(RunError::DependenciesNotPassed(key("implement")))
  );
  assert!(logged.run.resume_step(plan, at(22)).is_err());

  logged.apply(|run| run.decide_plan(plan, PlanDecision::ChangesRequested, at(22)));
  assert_eq!(status(&logged, "plan"), StepStatus::Running);
  assert!(logged.run.decide_plan(plan, PlanDecision::Approved, at(23)).is_err());

  logged.apply(|run| run.complete_step(plan, at(24)));
  logged.apply(|run| run.decide_plan(plan, PlanDecision::Approved, at(25)));
  assert_eq!(status(&logged, "plan"), StepStatus::Passed);
  logged.apply(|run| run.start_step(implement, at(26)));
}

#[test]
fn run_steps_run_succeeds_when_the_last_step_passes() {
  let mut logged = implemented();
  logged.pass("checks");
  logged.pass("review");
  assert_eq!(logged.run.status(), &RunStatus::Running);
  logged.pass("pr");
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  assert_eq!(logged.run.attempts()[0].status(), AttemptStatus::Succeeded);
  assert_eq!(
    logged.log.last().map(|event| event.kind()),
    Some(&RunEventKind::Succeeded)
  );
}

#[test]
fn run_steps_failure_routes_back_to_the_upstream_step_in_a_new_attempt() {
  let mut logged = implemented();
  let old_review = logged.exec("review");
  fail_checks(&mut logged);
  let run = &logged.run;
  assert_eq!(run.status(), &RunStatus::Running);
  assert_eq!(run.attempts().len(), 2);
  assert_eq!(run.attempts()[0].status(), AttemptStatus::RoutedBack);
  assert_eq!(run.execution(old_review).map(|e| e.status()), Some(StepStatus::Skipped));
  let retry = &run.attempts()[1];
  assert_eq!(
    retry.reason(),
    &AttemptReason::Retry {
      failed_step: key("checks"),
      return_to: key("implement"),
    }
  );
  let steps: Vec<_> = retry.executions().iter().map(|e| e.step().as_str()).collect();
  assert_eq!(steps, ["implement", "checks", "review", "pr"]);
  assert_eq!(status(&logged, "research"), StepStatus::Passed);
  assert_eq!(status(&logged, "plan"), StepStatus::Passed);
  assert_eq!(status(&logged, "implement"), StepStatus::Pending);
  assert_eq!(run.retries_taken(&key("checks")), 1);
}

#[test]
fn run_steps_executions_of_an_earlier_attempt_are_closed() {
  let mut logged = implemented();
  let old_checks = logged.exec("checks");
  fail_checks(&mut logged);
  assert_eq!(
    logged.run.start_step(old_checks, at(50)),
    Err(RunError::NotInCurrentAttempt(old_checks))
  );
}

#[test]
fn run_steps_retries_are_exhausted_after_max_retries() {
  let mut logged = implemented();
  fail_checks(&mut logged);
  logged.pass("implement");
  fail_checks(&mut logged);
  assert_eq!(logged.run.attempts().len(), 3);
  logged.pass("implement");
  fail_checks(&mut logged);
  let run = &logged.run;
  assert_eq!(run.attempts().len(), 3);
  assert_eq!(
    run.status(),
    &RunStatus::Failed(RunFailure::RetriesExhausted(key("checks")))
  );
  assert_eq!(run.attempts()[2].status(), AttemptStatus::Failed);
  assert!(
    run.attempts()[2]
      .executions()
      .iter()
      .all(|execution| !execution.status().is_open())
  );
}

#[test]
fn run_steps_failure_without_a_route_fails_the_run() {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.fail_step(research, StepFailure::AgentFailed, at(13)));
  assert_eq!(
    logged.run.status(),
    &RunStatus::Failed(RunFailure::StepFailed(key("research")))
  );
  assert_eq!(status(&logged, "plan"), StepStatus::Skipped);
  assert!(logged.run.resume(at(14)).is_err());
}

#[test]
fn run_steps_pending_step_cannot_fail() {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  assert!(matches!(
    logged.run.fail_step(research, StepFailure::AgentFailed, at(12)),
    Err(RunError::InvalidStepTransition {
      status: StepStatus::Pending,
      ..
    })
  ));
}

#[test]
fn run_steps_replay_matches_after_routing() {
  let mut logged = implemented();
  fail_checks(&mut logged);
  logged.pass("implement");
  assert_eq!(orchester_core::run::Run::replay(&logged.log), Ok(logged.run));
}
