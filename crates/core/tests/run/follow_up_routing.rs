//! A follow-up of a run that succeeded whose step reports the work failed (the project task ORC-44, after ORC-25 run
//! #23, whose two failed follow-ups left it succeeded): the run is routed as that step's failure is in any attempt —
//! back by its failure route, with the follow-up's report for the step the work returns to — or waits for a person
//! once the route's retries are used or where the step has none. It never stays succeeded. A follow-up that broke
//! leaves the run as it was (`conversation_a_failed_follow_up_leaves_the_run_as_it_was`).

use orchester_core::{
  id::StepExecutionId,
  orchestrator::StopReason,
  run::{AttemptReason, AttemptStatus, ReviewNote, Run, RunStatus, StepFailure, Stuck, StuckDecision},
};

use crate::fixture::{Config, Logged, at, key};

/// A run of `config`'s pipeline whose every step passed.
fn succeeded_with(config: &Config) -> Logged {
  let mut logged = Logged::pending_with(config);
  logged.apply(|run| run.start(at(11)));
  logged.through_plan();
  for step in ["implement", "checks", "review", "pr"] {
    logged.pass(step);
  }
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  logged
}

/// Follows up `step` of the succeeded run, whose agent reports the work failed; the follow-up's execution.
fn follow_up_fails(logged: &mut Logged, step: &str) -> StepExecutionId {
  logged.apply(|run| run.follow_up(&key(step), at(50)));
  let follow_up = logged.exec(step);
  logged.apply(|run| run.start_step(follow_up, at(51)));
  logged.apply(|run| run.fail_step(follow_up, StepFailure::ReportedFailed, at(52)));
  follow_up
}

fn planned(logged: &Logged) -> Vec<String> {
  logged
    .run
    .current_attempt()
    .expect("an attempt")
    .executions()
    .iter()
    .map(|execution| execution.step().as_str().to_owned())
    .collect()
}

#[test]
fn follow_up_routing_a_failed_verdict_returns_the_work_by_the_steps_route() {
  let mut logged = succeeded_with(&Config::feature_delivery());
  let follow_up = logged.run.attempts().len();
  let failed = follow_up_fails(&mut logged, "checks");
  assert_eq!(logged.run.status(), &RunStatus::Running);
  assert_eq!(logged.run.attempts()[follow_up].status(), AttemptStatus::RoutedBack);
  assert_eq!(
    logged.run.current_attempt().map(|attempt| attempt.reason()),
    Some(&AttemptReason::Retry {
      failed_step: key("checks"),
      return_to: key("implement"),
    })
  );
  assert_eq!(planned(&logged), ["implement", "checks", "review", "pr"]);
  // The step the work returns to reads what the follow-up reported.
  let report = logged
    .run
    .retry_report(logged.exec("implement"))
    .expect("implement learns why the work came back");
  assert_eq!(report.failed.id(), failed);
  assert_eq!(report.note, None);
  for step in ["implement", "checks", "review", "pr"] {
    logged.pass(step);
  }
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn follow_up_routing_waits_for_a_person_once_the_routes_retries_are_used() {
  let mut logged = Logged::pending_with(&Config::feature_delivery_with_retries(1));
  logged.apply(|run| run.start(at(11)));
  logged.through_plan();
  logged.pass("implement");
  let checks = logged.exec("checks");
  logged.apply(|run| run.start_step(checks, at(30)));
  logged.apply(|run| run.fail_step(checks, StepFailure::ReportedFailed, at(31)));
  for step in ["implement", "checks", "review", "pr"] {
    logged.pass(step);
  }
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  follow_up_fails(&mut logged, "checks");
  assert_eq!(
    logged.run.status(),
    &RunStatus::WaitingForPerson(Stuck::RetriesExhausted(key("checks")))
  );
  assert_eq!(logged.run.retry_from(), Some(&key("implement")));
  assert!(!logged.run.waits_without_route());
  assert_eq!(
    StopReason::of_run(&logged.run),
    Some(StopReason::RetriesExhausted { retries: 1 })
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn follow_up_routing_without_a_route_waits_with_the_return_choices() {
  let mut logged = succeeded_with(&Config::feature_delivery());
  let failed = follow_up_fails(&mut logged, "review");
  assert_eq!(
    logged.run.status(),
    &RunStatus::WaitingForPerson(Stuck::RetriesExhausted(key("review")))
  );
  // It took no retry: people are told the step failed, not that its retries ran out.
  assert!(logged.run.waits_without_route());
  assert_eq!(StopReason::of_run(&logged.run), Some(StopReason::NoRoute));
  assert_eq!(
    logged.run.return_targets(),
    ["research", "plan", "implement", "review"].map(key)
  );
  // Without a route, one more retry runs the step itself again.
  assert_eq!(logged.run.retry_from(), Some(&key("review")));
  assert_eq!(Run::replay(&logged.log), Ok(logged.run.clone()));

  let mut retried = logged.clone();
  retried.apply(|run| run.decide_stuck(&StuckDecision::Retry, at(60)));
  assert_eq!(
    retried.run.current_attempt().map(|attempt| attempt.reason()),
    Some(&AttemptReason::Retry {
      failed_step: key("review"),
      return_to: key("review"),
    })
  );
  assert_eq!(planned(&retried), ["review", "pr"]);

  let note = ReviewNote::new("Handle the empty body.").expect("a note");
  logged.apply(|run| {
    run.decide_stuck(
      &StuckDecision::ReturnTo {
        step: key("implement"),
        note: note.clone(),
      },
      at(60),
    )
  });
  assert_eq!(logged.run.status(), &RunStatus::Running);
  assert_eq!(planned(&logged), ["implement", "checks", "review", "pr"]);
  let report = logged
    .run
    .retry_report(logged.exec("implement"))
    .expect("implement learns why and what the person asks");
  assert_eq!((report.failed.id(), report.note), (failed, Some(&note)));
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn follow_up_routing_a_follow_up_that_could_not_verify_leaves_the_run_as_it_was() {
  let mut logged = succeeded_with(&Config::feature_delivery());
  logged.apply(|run| run.follow_up(&key("review"), at(50)));
  let follow_up = logged.exec("review");
  logged.apply(|run| run.start_step(follow_up, at(51)));
  logged.apply(|run| run.fail_step(follow_up, StepFailure::Unverified, at(52)));
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}
