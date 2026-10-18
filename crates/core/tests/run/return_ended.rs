//! Returning the work of an ended run to an agent step with a note (ORC-44): a run that succeeded or failed is
//! reopened in a new attempt from that step, whose execution carries the note; anything else is refused and the
//! run stays as it was.

use orchester_core::run::{
  AttemptReason, AttemptStatus, ReviewNote, Run, RunError, RunStatus, StepFailure, StepStatus, attempt_starts,
};

use crate::fixture::{Logged, at, key, stored};

fn note() -> ReviewNote {
  ReviewNote::new("Merge main into this branch and resolve the conflicts in schema.rs.").expect("a note")
}

fn succeeded() -> Logged {
  let mut logged = Logged::started();
  logged.through_plan();
  for step in ["implement", "checks", "review", "pr"] {
    logged.pass(step);
  }
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  logged
}

#[test]
fn return_ended_reopens_a_succeeded_run_from_the_step_with_the_note() {
  let mut logged = succeeded();
  let ended = logged.run.attempts().len();
  assert_eq!(
    logged.run.ended_return_targets(),
    ["research", "plan", "implement", "checks", "review", "pr"].map(key)
  );
  logged.apply(|run| run.return_ended(&key("implement"), &note(), at(50)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
  assert_eq!(logged.run.attempts().len(), ended + 1);
  assert_eq!(logged.run.attempts()[ended - 1].status(), AttemptStatus::Succeeded);
  let attempt = logged.run.current_attempt().expect("the new attempt");
  assert_eq!(
    attempt.reason(),
    &AttemptReason::ReturnedAfterEnd {
      return_to: key("implement"),
      note: note(),
    }
  );
  let steps: Vec<&str> = attempt
    .executions()
    .iter()
    .map(|execution| execution.step().as_str())
    .collect();
  assert_eq!(steps, ["implement", "checks", "review", "pr"]);
  assert_eq!(logged.run.returned_note(logged.exec("implement")), Some(&note()));
  assert_eq!(logged.run.returned_note(logged.exec("checks")), None);
  assert!(attempt_starts(&logged.log).last().is_some_and(|start| start.by_person));
  for step in ["implement", "checks", "review", "pr"] {
    logged.pass(step);
  }
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run.clone()));
  assert_eq!(Run::restore(stored(&logged.run)), Ok(logged.run));
}

#[test]
fn return_ended_reopens_a_failed_run() {
  let mut logged = Logged::started();
  logged.through_plan();
  let execution = logged.exec("implement");
  logged.apply(|run| run.start_step(execution, at(30)));
  logged.apply(|run| run.fail_step(execution, StepFailure::ReportedFailed, at(31)));
  assert!(matches!(logged.run.status(), RunStatus::Failed(_)));
  logged.apply(|run| run.return_ended(&key("plan"), &note(), at(40)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
  assert_eq!(
    logged.run.latest_execution(&key("plan")).map(|step| step.status()),
    Some(StepStatus::Pending)
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn return_ended_refuses_a_run_that_has_not_ended_and_a_step_that_never_ran() {
  let mut running = Logged::started();
  running.through_plan();
  let before = running.run.clone();
  assert!(running.run.ended_return_targets().is_empty());
  assert!(matches!(
    running.run.return_ended(&key("plan"), &note(), at(40)),
    Err(RunError::InvalidRunTransition { .. })
  ));
  assert_eq!(running.run, before);

  let mut failed = Logged::started();
  failed.through_plan();
  let execution = failed.exec("implement");
  failed.apply(|run| run.start_step(execution, at(30)));
  failed.apply(|run| run.fail_step(execution, StepFailure::ReportedFailed, at(31)));
  let before = failed.run.clone();
  assert_eq!(
    failed.run.return_ended(&key("review"), &note(), at(40)),
    Err(RunError::NotAReturnTarget(key("review")))
  );
  assert_eq!(failed.run, before);
}

#[test]
fn return_ended_a_cancelled_run_is_not_returned() {
  let mut logged = Logged::started();
  logged.apply(|run| run.cancel(at(20)));
  assert!(!logged.run.can_return_ended());
  assert!(logged.run.return_ended(&key("research"), &note(), at(30)).is_err());
}
