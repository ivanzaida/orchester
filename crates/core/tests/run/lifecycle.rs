use std::{num::NonZeroU32, time::Duration};

use orchester_core::{
  id::RunId,
  run::{
    AttemptReason, AttemptStatus, PauseReason, Run, RunError, RunEventKind, RunNumber, RunStatus, StepStatus, StoredRun,
  },
  tokens::TokensUsed,
};

use crate::fixture::{Config, Logged, at, new_run, revision, stored};

const PAUSED: RunStatus = RunStatus::Paused(PauseReason::Requested);

#[test]
fn run_lifecycle_start_opens_attempt_one_with_every_step_pending() {
  let logged = Logged::started();
  let run = &logged.run;
  assert_eq!(run.status(), &RunStatus::Running);
  assert_eq!(run.attempts().len(), 1);
  let attempt = &run.attempts()[0];
  assert_eq!(attempt.number().get(), 1);
  assert_eq!(attempt.reason(), &AttemptReason::Initial);
  assert_eq!(attempt.status(), AttemptStatus::Active);
  assert_eq!(attempt.executions().len(), 6);
  assert!(
    attempt
      .executions()
      .iter()
      .all(|execution| execution.status() == StepStatus::Pending)
  );
  let kinds: Vec<_> = logged.log.iter().map(|event| event.kind().name()).collect();
  assert_eq!(kinds, ["created", "started", "attempt_started"]);
  let sequences: Vec<_> = logged.log.iter().map(|event| event.sequence().get()).collect();
  assert_eq!(sequences, [1, 2, 3]);
}

#[test]
fn run_lifecycle_create_rejects_a_zero_time_limit() {
  let mut new = new_run(Config::feature_delivery().snapshot());
  new.max_duration = Duration::ZERO;
  let number = RunNumber::new(NonZeroU32::MIN);
  assert_eq!(
    Run::create(RunId::generate(), number, new, at(1)).map(|_| ()),
    Err(RunError::ZeroDuration)
  );
}

#[test]
fn run_lifecycle_start_is_only_valid_for_a_pending_run() {
  let mut logged = Logged::started();
  assert!(matches!(
    logged.run.start(at(12)),
    Err(RunError::InvalidRunTransition {
      status: RunStatus::Running,
      event: "started"
    })
  ));
}

#[test]
fn run_lifecycle_pause_and_resume_alternate() {
  let mut logged = Logged::started();
  logged.apply(|run| run.pause(at(12)));
  assert_eq!(logged.run.status(), &PAUSED);
  assert!(matches!(
    logged.run.pause(at(13)),
    Err(RunError::InvalidRunTransition { .. })
  ));
  logged.apply(|run| run.resume(at(14)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
  assert!(matches!(
    logged.run.resume(at(15)),
    Err(RunError::InvalidRunTransition { .. })
  ));
}

#[test]
fn run_lifecycle_pending_run_cannot_pause_interrupt_or_continue() {
  let mut logged = Logged::pending();
  assert!(logged.run.pause(at(12)).is_err());
  assert!(
    logged
      .run
      .interrupt(orchester_core::run::InterruptCause::Crashed, at(12))
      .is_err()
  );
  assert!(logged.run.continue_run(at(12)).is_err());
  assert_eq!(logged.run.status(), &RunStatus::Pending);
}

#[test]
fn run_lifecycle_steps_do_not_move_while_paused() {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  logged.apply(|run| run.pause(at(12)));
  assert!(matches!(
    logged.run.start_step(research, at(13)),
    Err(RunError::InvalidRunTransition { status: PAUSED, .. })
  ));
}

#[test]
fn run_lifecycle_cancel_skips_open_executions_and_is_final() {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.pause(at(13)));
  logged.apply(|run| run.cancel(at(14)));
  let run = &logged.run;
  assert_eq!(run.status(), &RunStatus::Cancelled);
  assert_eq!(run.attempts()[0].status(), AttemptStatus::Cancelled);
  assert!(
    run.attempts()[0]
      .executions()
      .iter()
      .all(|execution| execution.status() == StepStatus::Skipped)
  );
  assert!(matches!(
    logged.run.cancel(at(15)),
    Err(RunError::InvalidRunTransition { .. })
  ));
  assert!(logged.run.resume(at(15)).is_err());
}

#[test]
fn run_lifecycle_pending_run_can_be_cancelled() {
  let mut logged = Logged::pending();
  logged.apply(|run| run.cancel(at(12)));
  assert_eq!(logged.run.status(), &RunStatus::Cancelled);
  assert!(logged.run.attempts().is_empty());
}

/// A run cancelled before its first attempt is a finished run like any other: its stored state
/// and its event log both read back.
#[test]
fn run_lifecycle_run_cancelled_before_it_started_restores_and_replays() {
  let mut logged = Logged::pending();
  logged.apply(|run| run.cancel(at(12)));
  let kinds: Vec<_> = logged.log.iter().map(|event| event.kind().name()).collect();
  assert_eq!(kinds, ["created", "cancelled"]);
  assert_eq!(Run::restore(stored(&logged.run)), Ok(logged.run.clone()));
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn run_lifecycle_restore_rejects_attempts_that_contradict_the_status() {
  let started = Logged::started().run;
  let pending = StoredRun {
    status: RunStatus::Pending,
    ..stored(&started)
  };
  assert!(matches!(Run::restore(pending), Err(RunError::Inconsistent(_))));
  let unstarted = stored(&Logged::pending().run);
  for status in [RunStatus::Running, PAUSED, RunStatus::Succeeded] {
    let never_ran = StoredRun {
      status: status.clone(),
      ..unstarted.clone()
    };
    assert!(
      matches!(Run::restore(never_ran), Err(RunError::Inconsistent(_))),
      "{status:?}"
    );
  }
}

#[test]
fn run_lifecycle_usage_accumulates_in_every_status() {
  let mut logged = Logged::started();
  logged.apply(|run| run.record_usage(TokensUsed::new(250_000), at(12)));
  logged.apply(|run| run.pause(at(13)));
  logged.apply(|run| run.record_usage(TokensUsed::new(12_480), at(14)));
  assert_eq!(logged.run.used(), TokensUsed::new(262_480));
  assert!(!logged.run.token_limit_reached());
}

#[test]
fn run_lifecycle_checkpoints_need_an_active_step() {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  assert!(matches!(
    logged.run.record_checkpoint(research, revision('b'), at(12)),
    Err(RunError::InvalidStepTransition {
      status: StepStatus::Pending,
      ..
    })
  ));
  logged.apply(|run| run.start_step(research, at(13)));
  logged.apply(|run| run.record_checkpoint(research, revision('b'), at(14)));
  let checkpoint = logged.run.last_checkpoint().expect("recorded");
  assert_eq!(checkpoint.execution(), research);
  assert_eq!(checkpoint.revision(), &revision('b'));
  assert_eq!(checkpoint.created_at(), at(14));
}

#[test]
fn run_lifecycle_rejected_transition_leaves_the_run_unchanged() {
  let mut logged = Logged::started();
  let before = logged.run.clone();
  let pr = logged.exec("pr");
  assert_eq!(
    logged.run.start_step(pr, at(12)),
    Err(RunError::DependenciesNotPassed(crate::fixture::key("pr")))
  );
  assert_eq!(logged.run, before);
}

#[test]
fn run_lifecycle_replay_rebuilds_the_state_from_events() {
  let mut logged = Logged::started();
  logged.through_plan();
  let implement = logged.exec("implement");
  logged.apply(|run| run.start_step(implement, at(30)));
  logged.apply(|run| run.record_checkpoint(implement, revision('c'), at(31)));
  logged.apply(|run| run.record_usage(TokensUsed::new(42), at(32)));
  logged.apply(|run| run.interrupt(orchester_core::run::InterruptCause::QuitNow, at(33)));
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn run_lifecycle_replay_rejects_gaps_foreign_events_and_a_missing_creation() {
  let logged = Logged::started();
  let mut gap = logged.log.clone();
  gap.remove(1);
  assert!(matches!(Run::replay(&gap), Err(RunError::OutOfSequence { .. })));
  assert_eq!(Run::replay(&logged.log[1..]), Err(RunError::Creation));
  assert_eq!(Run::replay(&[]), Err(RunError::Creation));
  let other = Logged::started();
  let mut foreign = logged.log;
  foreign[2] = other.log[2].clone();
  assert!(matches!(Run::replay(&foreign), Err(RunError::WrongRun { .. })));
}

#[test]
fn run_lifecycle_replay_rechecks_every_transition() {
  let mut log = Logged::started().log;
  log.push(orchester_core::run::RunEvent::new(
    log[1].run(),
    log[2].sequence().next(),
    at(12),
    RunEventKind::Started,
  ));
  assert!(matches!(Run::replay(&log), Err(RunError::InvalidRunTransition { .. })));
}
