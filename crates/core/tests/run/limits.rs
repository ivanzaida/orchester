use std::time::Duration;

use orchester_core::{
  connection::stream::InvocationOutcome,
  id::{InvocationId, RunId},
  run::{
    Invocation, InvocationKind, PauseReason, RaisedLimits, Run, RunError, RunLimit, RunStatus, StepStatus, TaskPhase,
    active_time,
  },
  task::{Task, TaskDetails, TaskStatus},
  tokens::TokensUsed,
};

use crate::fixture::{Logged, at, model, revision, tokens};

const MINUTE: Duration = Duration::from_secs(60);

/// A run whose `research` step is running and has used `used` tokens.
fn working(used: u64) -> Logged {
  let mut logged = Logged::started();
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.record_usage(TokensUsed::new(used), at(13)));
  logged
}

/// `working`, stopped at its token limit with a checkpoint of what research changed.
fn stopped_at_token_limit() -> Logged {
  let mut logged = working(1_400_000);
  assert!(logged.run.token_limit_reached());
  let research = logged.exec("research");
  logged.apply(|run| run.record_checkpoint(research, revision('d'), at(14)));
  logged.apply(|run| run.stop_at_limit(RunLimit::Tokens, at(15)));
  logged
}

fn raised(budget: u64, minutes: u64) -> RaisedLimits {
  RaisedLimits {
    budget: tokens(budget),
    max_duration: MINUTE * u32::try_from(minutes).expect("few minutes"),
  }
}

#[test]
fn run_limits_pause_is_requested_first_and_claimed_at_the_checkpoint() {
  let mut logged = working(1_000);
  logged.apply(|run| run.request_pause(at(14)));
  assert!(logged.run.pause_requested());
  assert_eq!(logged.run.status(), &RunStatus::Running);
  // Asking again while it settles changes nothing.
  assert_eq!(logged.run.request_pause(at(15)), Ok(Vec::new()));
  let research = logged.exec("research");
  logged.apply(|run| run.complete_step(research, at(16)));
  logged.apply(|run| run.pause(at(17)));
  assert_eq!(logged.run.status(), &RunStatus::Paused(PauseReason::Requested));
  assert!(!logged.run.pause_requested());
  logged.apply(|run| run.resume(at(18)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn run_limits_cancel_ends_a_requested_pause() {
  let mut logged = working(1_000);
  logged.apply(|run| run.request_pause(at(14)));
  logged.apply(|run| run.cancel(at(15)));
  assert_eq!(logged.run.status(), &RunStatus::Cancelled);
  assert!(!logged.run.pause_requested());
  assert!(logged.run.request_pause(at(16)).is_err());
}

#[test]
fn run_limits_stop_at_limit_keeps_the_step_open_and_waits_for_a_raise() {
  let mut logged = stopped_at_token_limit();
  let run = &logged.run;
  assert_eq!(
    run.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens))
  );
  let research = run.execution(logged.exec("research")).expect("execution");
  assert_eq!(research.status(), StepStatus::Running);
  assert_eq!(run.last_checkpoint().map(|c| c.revision()), Some(&revision('d')));
  let task = Task::draft(
    run.task(),
    orchester_core::project::TaskKey::new(
      orchester_core::project::ProjectKey::new("ORC").expect("key"),
      std::num::NonZeroU32::MIN,
    ),
    TaskDetails {
      title: orchester_core::name::Name::new("Task").expect("name"),
      brief: String::new(),
      acceptance_criteria: Vec::new(),
      pipeline: None,
      limits: None,
    },
    at(1),
  );
  assert_eq!(task.status(), TaskStatus::Draft);
  assert_eq!(TaskPhase::of(&task, Some(run), None), TaskPhase::StoppedAtLimit);
  assert!(!TaskPhase::StoppedAtLimit.is_active());
  assert_eq!(
    logged.run.resume(at(16)),
    Err(RunError::LimitNotRaised(RunLimit::Tokens))
  );
}

#[test]
fn run_limits_raise_continues_the_run_with_the_new_limits() {
  let mut logged = stopped_at_token_limit();
  let max = tokens(10_000_000);
  logged.apply(|run| run.continue_with_limits(raised(2_000_000, 30), MINUTE * 24, max, at(20)));
  let run = &logged.run;
  assert_eq!(run.status(), &RunStatus::Running);
  assert_eq!(run.budget(), tokens(2_000_000));
  assert_eq!(run.created().budget, tokens(1_400_000));
  assert!(!run.token_limit_reached());
  let kinds: Vec<_> = logged.log[logged.log.len() - 2..]
    .iter()
    .map(|event| event.kind().name())
    .collect();
  assert_eq!(kinds, ["limit_raised", "resumed"]);
  assert_eq!(Run::replay(&logged.log), Ok(logged.run));
}

#[test]
fn run_limits_raise_must_exceed_what_was_used_and_fit_the_maximum() {
  let mut logged = stopped_at_token_limit();
  let before = logged.run.clone();
  let max = tokens(10_000_000);
  let active = MINUTE * 24;
  assert_eq!(
    logged
      .run
      .continue_with_limits(raised(1_400_000, 30), active, max, at(20)),
    Err(RunError::LimitNotAboveUsed(RunLimit::Tokens))
  );
  assert_eq!(
    logged
      .run
      .continue_with_limits(raised(2_000_000, 24), active, max, at(20)),
    Err(RunError::LimitNotAboveUsed(RunLimit::Time))
  );
  assert_eq!(
    logged
      .run
      .continue_with_limits(raised(12_000_000, 30), active, max, at(20)),
    Err(RunError::BudgetAboveMaximum { max })
  );
  assert_eq!(logged.run, before);
  let mut running = working(10);
  assert!(matches!(
    running
      .run
      .continue_with_limits(raised(2_000_000, 30), active, max, at(20)),
    Err(RunError::InvalidRunTransition { .. })
  ));
  assert!(running.run.stop_at_limit(RunLimit::Time, at(21)).is_ok());
}

#[test]
fn run_limits_time_counts_active_agent_time_only() {
  let run = RunId::generate();
  let execution = working(0).exec("research");
  let invocation = |started: i64, ended: Option<i64>| Invocation {
    id: InvocationId::generate(),
    run,
    execution,
    number: 1,
    kind: InvocationKind::Agent {
      model: model("gpt-5.5"),
    },
    session: None,
    started_at: at(started),
    ended_at: ended.map(at),
    usage: orchester_core::run::TokenUsage::default(),
    outcome: ended.map(|_| InvocationOutcome::Terminated),
  };
  // Ten minutes of work, a pause of an hour, then five minutes of a running invocation.
  let minute = 60_000;
  let invocations = [invocation(0, Some(10 * minute)), invocation(70 * minute, None)];
  assert_eq!(active_time(&invocations, at(75 * minute)), MINUTE * 15);
  let logged = working(0);
  assert!(!logged.run.time_limit_reached(MINUTE * 29));
  assert!(logged.run.time_limit_reached(MINUTE * 30));
}

#[test]
fn run_limits_a_daily_budget_stop_resumes_without_a_raise() {
  let mut logged = working(700);
  logged.apply(|run| run.stop_at_limit(RunLimit::DailyBudget, at(14)));
  assert_eq!(
    logged.run.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::DailyBudget))
  );
  logged.apply(|run| run.resume(at(15)));
  assert_eq!(logged.run.status(), &RunStatus::Running);
}
