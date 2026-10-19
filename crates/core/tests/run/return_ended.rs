//! Returning the work of an ended run to an agent step with a note (the project task ORC-44 · Return a finished run
//! to an earlier step): a run that succeeded or failed is reopened in a new attempt from that step, whose execution
//! carries the note and the conflicts it resolves; anything else is refused and the run stays as it was. A log that
//! reopens a failed run for anything but a return does not replay.

use std::num::{NonZeroU32, NonZeroU64};

use orchester_core::{
  id::{AttemptId, StepExecutionId},
  run::{
    AttemptReason, AttemptStatus, BranchName, EventSequence, MergeConflicts, PlannedExecution, ReviewNote, Run,
    RunError, RunEvent, RunEventKind, RunStatus, StepFailure, StepStatus, attempt_starts,
  },
};

use crate::fixture::{Logged, at, key, stored};

fn note() -> ReviewNote {
  ReviewNote::new("Merge main into this branch and resolve the conflicts listed below.").expect("a note")
}

fn conflicts() -> MergeConflicts {
  MergeConflicts {
    base: BranchName::new("main").expect("a branch"),
    files: vec!["crates/store/src/schema.rs".to_owned()],
  }
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

fn failed() -> Logged {
  let mut logged = Logged::started();
  logged.through_plan();
  let execution = logged.exec("implement");
  logged.apply(|run| run.start_step(execution, at(30)));
  logged.apply(|run| run.fail_step(execution, StepFailure::ReportedFailed, at(31)));
  assert!(matches!(logged.run.status(), RunStatus::Failed(_)));
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
  logged.apply(|run| run.return_ended(&key("implement"), (&note(), Some(&conflicts())), at(50)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
  assert_eq!(logged.run.attempts().len(), ended + 1);
  assert_eq!(logged.run.attempts()[ended - 1].status(), AttemptStatus::Succeeded);
  let attempt = logged.run.current_attempt().expect("the new attempt");
  assert_eq!(
    attempt.reason(),
    &AttemptReason::ReturnedAfterEnd {
      return_to: key("implement"),
      note: note(),
      conflicts: Some(conflicts()),
    }
  );
  let steps: Vec<&str> = attempt
    .executions()
    .iter()
    .map(|execution| execution.step().as_str())
    .collect();
  assert_eq!(steps, ["implement", "checks", "review", "pr"]);
  let returned = conflicts();
  assert_eq!(
    logged.run.returned_work(logged.exec("implement")),
    Some((&note(), Some(&returned)))
  );
  assert_eq!(logged.run.returned_work(logged.exec("checks")), None);
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
  let mut logged = failed();
  logged.apply(|run| run.return_ended(&key("plan"), (&note(), None), at(40)));
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
    running.run.return_ended(&key("plan"), (&note(), None), at(40)),
    Err(RunError::InvalidRunTransition { .. })
  ));
  assert_eq!(running.run, before);

  let mut failed = failed();
  let before = failed.run.clone();
  assert_eq!(
    failed.run.return_ended(&key("review"), (&note(), None), at(40)),
    Err(RunError::NotAReturnTarget(key("review")))
  );
  assert_eq!(failed.run, before);
}

#[test]
fn return_ended_a_cancelled_run_is_not_returned() {
  let mut logged = Logged::started();
  logged.apply(|run| run.cancel(at(20)));
  assert!(!logged.run.can_return_ended());
  assert!(
    logged
      .run
      .return_ended(&key("research"), (&note(), None), at(30))
      .is_err()
  );
}

#[test]
fn return_ended_a_log_that_reopens_a_failed_run_for_a_retry_does_not_replay() {
  let logged = failed();
  let mut log = logged.log.clone();
  let next = |log: &[RunEvent]| {
    let last = log.last().expect("events").sequence().get();
    EventSequence::new(NonZeroU64::new(last + 1).expect("positive"))
  };
  log.push(RunEvent::new(
    logged.run.id(),
    next(&log),
    at(40),
    RunEventKind::Reopened,
  ));
  log.push(RunEvent::new(
    logged.run.id(),
    next(&log),
    at(40),
    RunEventKind::AttemptStarted {
      attempt: AttemptId::generate(),
      number: NonZeroU32::new(2).expect("positive"),
      reason: AttemptReason::Retry {
        failed_step: key("implement"),
        return_to: key("implement"),
      },
      executions: ["implement", "checks", "review", "pr"]
        .map(|step| PlannedExecution {
          id: StepExecutionId::generate(),
          step: key(step),
        })
        .to_vec(),
    },
  ));
  assert!(matches!(Run::replay(&log), Err(RunError::InvalidAttempt(_))));
}
