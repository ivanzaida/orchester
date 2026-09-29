use orchester_core::run::{AttemptReason, AttemptStatus, Run, RunStatus, StepFailure, StepStatus};

use crate::fixture::{Logged, at, key, revision, stored};

fn failed() -> Logged {
  let mut logged = Logged::started();
  logged.through_plan();
  let execution = logged.exec("implement");
  logged.apply(|run| run.start_step(execution, at(30)));
  logged.apply(|run| run.record_checkpoint(execution, revision('b'), at(31)));
  logged.apply(|run| run.fail_step(execution, StepFailure::ReportedFailed, at(32)));
  logged
}

#[test]
fn failed_recovery_keeps_history_and_upstream_work_and_replays() {
  let mut logged = failed();
  let old = logged.run.clone();
  let previous = logged.exec("implement");
  assert_eq!(logged.run.recovery_step(None), Some(key("implement")));
  logged.apply(|run| run.recover(&key("implement"), at(40)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
  assert_eq!(logged.run.id(), old.id());
  assert_eq!(logged.run.branch(), old.branch());
  assert_eq!(logged.run.last_checkpoint(), old.last_checkpoint());
  assert_eq!(logged.run.attempts()[0].status(), AttemptStatus::Failed);
  assert_eq!(
    logged.run.latest_execution(&key("plan")).map(|step| step.status()),
    Some(StepStatus::Passed)
  );
  assert_eq!(logged.run.recovered_execution(logged.exec("implement")), Some(previous));
  assert_eq!(
    logged.run.current_attempt().expect("attempt").reason(),
    &AttemptReason::Recovery { step: key("implement") }
  );
  for step in ["implement", "checks", "review", "pr"] {
    logged.pass(step);
  }
  assert_eq!(logged.run.status(), &RunStatus::Succeeded);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run.clone()));
  assert_eq!(Run::restore(stored(&logged.run)), Ok(logged.run));
}

#[test]
fn failed_recovery_refuses_unexecuted_agent_and_nonfailed_runs_atomically() {
  let mut logged = failed();
  let before = logged.run.clone();
  assert!(logged.run.recover(&key("review"), at(40)).is_err());
  assert_eq!(logged.run, before);
  logged.apply(|run| run.recover(&key("implement"), at(41)));
  let running = logged.run.clone();
  assert!(logged.run.recover(&key("implement"), at(42)).is_err());
  assert_eq!(logged.run, running);
  logged.apply(|run| run.cancel(at(43)));
  assert!(logged.run.recover(&key("implement"), at(44)).is_err());
}

#[test]
fn failed_recovery_a_second_failure_is_still_recoverable_without_erasing_the_first() {
  let mut logged = failed();
  logged.apply(|run| run.recover(&key("implement"), at(40)));
  let retry = logged.exec("implement");
  logged.apply(|run| run.start_step(retry, at(41)));
  logged.apply(|run| run.fail_step(retry, StepFailure::ReportedFailed, at(42)));
  logged.apply(|run| run.recover(&key("implement"), at(43)));
  assert_eq!(logged.run.recovered_execution(logged.exec("implement")), Some(retry));
  assert_eq!(logged.run.attempts().len(), 3);
  assert!(
    logged.run.attempts()[..2]
      .iter()
      .all(|attempt| attempt.status() == AttemptStatus::Failed)
  );
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}
