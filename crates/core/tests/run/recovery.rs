use orchester_core::run::{ContinueTrigger, InterruptCause, Run, RunError, RunEventKind, RunStatus, StepStatus};

use crate::fixture::{Logged, at, revision};

/// A run interrupted by `cause` while `research` was running past checkpoint `b`.
fn interrupted(cause: InterruptCause) -> Logged {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.record_checkpoint(research, revision('b'), at(13)));
  logged.apply(|run| run.interrupt(cause, at(14)));
  logged
}

#[test]
fn run_recovery_interruption_keeps_the_checkpoint_and_claims_no_progress() {
  let logged = interrupted(InterruptCause::Crashed);
  let run = &logged.run;
  assert_eq!(run.status(), &RunStatus::Interrupted(InterruptCause::Crashed));
  assert_eq!(run.last_checkpoint().map(|c| c.revision()), Some(&revision('b')));
  let research = run.execution(logged.exec("research")).expect("execution");
  assert_eq!(research.status(), StepStatus::Running);
}

#[test]
fn run_recovery_only_a_running_run_is_interrupted() {
  let mut logged = Logged::started();
  logged.apply(|run| run.pause(at(12)));
  assert!(matches!(
    logged.run.interrupt(InterruptCause::QuitNow, at(13)),
    Err(RunError::InvalidRunTransition {
      status: RunStatus::Paused,
      ..
    })
  ));
}

#[test]
fn run_recovery_continue_resumes_from_the_last_checkpoint() {
  let mut logged = interrupted(InterruptCause::QuitNow);
  let checkpoint = logged.run.last_checkpoint().expect("checkpoint").id();
  let before = logged.log.len();
  logged.apply(|run| run.continue_run(at(20)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
  assert_eq!(
    logged.log[before].kind(),
    &RunEventKind::Continued {
      trigger: ContinueTrigger::User,
      from: Some(checkpoint),
    }
  );
  assert_eq!(logged.run.last_checkpoint().map(|c| c.id()), Some(checkpoint));
  assert!(matches!(
    logged.run.continue_run(at(21)),
    Err(RunError::InvalidRunTransition { .. })
  ));
}

#[test]
fn run_recovery_runner_loss_continues_on_reconnect_once() {
  let mut logged = interrupted(InterruptCause::RunnerLost);
  assert!(logged.run.continues_on_reconnect());
  let events = logged.run.continue_after_reconnect(at(20)).expect("continued");
  assert_eq!(events.len(), 1);
  assert!(matches!(
    events[0].kind(),
    RunEventKind::Continued {
      trigger: ContinueTrigger::RunnerReconnected,
      from: Some(_),
    }
  ));
  logged.log.extend(events);
  assert_eq!(logged.run.status(), &RunStatus::Running);
  assert!(!logged.run.continues_on_reconnect());
  assert_eq!(logged.run.continue_after_reconnect(at(21)), Ok(Vec::new()));
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn run_recovery_session_end_waits_for_an_explicit_continue() {
  for cause in [InterruptCause::QuitNow, InterruptCause::Crashed] {
    let mut logged = interrupted(cause);
    assert!(!logged.run.continues_on_reconnect());
    assert_eq!(
      logged.run.continue_after_reconnect(at(20)),
      Err(RunError::NeedsExplicitContinue(cause))
    );
    assert_eq!(logged.run.status(), &RunStatus::Interrupted(cause));
    logged.apply(|run| run.continue_run(at(21)));
    assert_eq!(logged.run.status(), &RunStatus::Running);
  }
}

#[test]
fn run_recovery_without_a_checkpoint_continues_from_the_start_of_the_step() {
  let mut logged = Logged::started();
  logged.apply(|run| run.interrupt(InterruptCause::RunnerLost, at(12)));
  let events = logged.run.continue_after_reconnect(at(13)).expect("continued");
  assert!(matches!(events[0].kind(), RunEventKind::Continued { from: None, .. }));
}

#[test]
fn run_recovery_interrupted_run_can_be_cancelled_keeping_its_checkpoint() {
  let mut logged = interrupted(InterruptCause::Crashed);
  logged.apply(|run| run.cancel(at(20)));
  assert_eq!(logged.run.status(), &RunStatus::Cancelled);
  assert_eq!(logged.run.last_checkpoint().map(|c| c.revision()), Some(&revision('b')));
}
