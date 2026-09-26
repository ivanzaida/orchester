//! A step that uses its budget stops the run at its limit ("Stopped at limit", step budget): no
//! failure, no retry; raising that step's budget for the run continues it.

use orchester_core::{
  run::{PauseReason, RaisedLimits, Run, RunError, RunLimit, RunStatus, StepStatus},
  tokens::TokensUsed,
};

use crate::fixture::{Logged, at, key, revision, tokens};

const MAX: u64 = 10_000_000;

/// A run whose `research` step used its budget: its work was committed and the run stopped.
fn stopped_at_step_budget() -> (Logged, RunLimit) {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.record_usage(TokensUsed::new(400_500), at(13)));
  logged.apply(|run| run.record_checkpoint(research, revision('d'), at(14)));
  let limit = RunLimit::StepBudget { execution: research };
  logged.apply(|run| run.stop_at_limit(limit, at(15)));
  (logged, limit)
}

#[test]
fn step_budget_stop_pauses_the_run_without_failing_the_step() {
  let (mut logged, limit) = stopped_at_step_budget();
  let run = &logged.run;
  assert_eq!(run.status(), &RunStatus::Paused(PauseReason::LimitReached(limit)));
  let research = run.execution(logged.exec("research")).expect("execution");
  assert_eq!(research.status(), StepStatus::Running);
  assert_eq!(research.failure(), None);
  assert_eq!(run.attempts().len(), 1);
  assert_eq!(run.retries_taken(&key("research")), 0);
  assert!(!limit.is_run_limit());
  assert_eq!(logged.run.resume(at(16)), Err(RunError::LimitNotRaised(limit)));
  let run_limits = RaisedLimits {
    budget: tokens(2_000_000),
    max_duration: std::time::Duration::from_secs(3_600),
  };
  assert!(matches!(
    logged
      .run
      .continue_with_limits(run_limits, std::time::Duration::ZERO, tokens(MAX), at(16)),
    Err(RunError::InvalidRunTransition { .. })
  ));
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn step_budget_raise_applies_to_the_run_and_continues_it() {
  let (mut logged, _) = stopped_at_step_budget();
  let used = TokensUsed::new(400_500);
  assert_eq!(logged.run.step_budget(&key("research")), None);
  logged.apply(|run| run.raise_step_budget(tokens(600_000), used, tokens(MAX), at(20)));
  let run = &logged.run;
  assert_eq!(run.status(), &RunStatus::Running);
  assert_eq!(run.step_budget(&key("research")), Some(tokens(600_000)));
  assert_eq!(run.budget(), tokens(1_400_000), "the run's own budget is unchanged");
  let research = run.execution(logged.exec("research")).expect("execution");
  assert_eq!(research.status(), StepStatus::Running, "the same execution continues");
  assert_eq!(run.attempts().len(), 1);
  let kinds: Vec<_> = logged.log[logged.log.len() - 2..]
    .iter()
    .map(|event| event.kind().name())
    .collect();
  assert_eq!(kinds, ["step_budget_raised", "resumed"]);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn step_budget_raise_must_exceed_what_the_step_used_and_fit_the_maximum() {
  let (mut logged, limit) = stopped_at_step_budget();
  let before = logged.run.clone();
  let used = TokensUsed::new(400_500);
  assert_eq!(
    logged.run.raise_step_budget(tokens(400_500), used, tokens(MAX), at(20)),
    Err(RunError::LimitNotAboveUsed(limit))
  );
  assert_eq!(
    logged
      .run
      .raise_step_budget(tokens(12_000_000), used, tokens(MAX), at(20)),
    Err(RunError::BudgetAboveMaximum { max: tokens(MAX) })
  );
  assert_eq!(logged.run, before);

  // Only a run stopped at a step's budget takes a raised step budget.
  let mut paused = Logged::started();
  let research = paused.exec("research");
  paused.apply(|run| run.start_step(research, at(12)));
  paused.apply(|run| run.stop_at_limit(RunLimit::Tokens, at(13)));
  assert!(matches!(
    paused.run.raise_step_budget(tokens(600_000), used, tokens(MAX), at(20)),
    Err(RunError::InvalidRunTransition { .. })
  ));
}

#[test]
fn step_budget_stop_names_an_open_execution_of_the_current_attempt() {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.complete_step(research, at(13)));
  let passed = RunLimit::StepBudget { execution: research };
  assert!(matches!(
    logged.run.stop_at_limit(passed, at(14)),
    Err(RunError::InvalidStepTransition { .. })
  ));
}
