use std::{num::NonZeroU32, time::Duration};

use orchester_core::{
  id::RunId,
  limits::ProjectLimits,
  run::{
    BranchName, InterruptCause, Run, RunAgainBlock, RunAgainInput, RunNumber, StepFailure, can_run_again_after,
    holds_task, run_again,
  },
  tokens::TokensUsed,
};

use crate::fixture::{Config, Logged, at, new_run, task_key, tokens};

fn run(number: u32) -> Logged {
  let (run, created) = Run::create(
    RunId::generate(),
    RunNumber::new(NonZeroU32::new(number).expect("positive")),
    new_run(Config::feature_delivery().snapshot()),
    at(10),
  )
  .expect("positive time limit");
  let mut logged = Logged {
    run,
    log: vec![created],
  };
  logged.apply(|run| run.start(at(11)));
  logged
}

fn failed(number: u32) -> Run {
  let mut logged = run(number);
  let research = logged.exec("research");
  logged.apply(|run| run.start_step(research, at(12)));
  logged.apply(|run| run.fail_step(research, StepFailure::ReportedFailed, at(13)));
  logged.run
}

fn cancelled(number: u32) -> Run {
  let mut logged = run(number);
  logged.apply(|run| run.cancel(at(12)));
  logged.run
}

fn input<'a>(runs: &'a [Option<&'a Run>], after: RunId) -> RunAgainInput<'a> {
  RunAgainInput {
    runs,
    after,
    worktree_removed: &[],
    pipeline_published: true,
    limits: ProjectLimits::DEFAULT,
    used_today: TokensUsed::ZERO,
  }
}

#[test]
fn run_again_follows_a_failed_or_cancelled_latest_run() {
  let failed = failed(131);
  assert_eq!(run_again(&input(&[Some(&failed)], failed.id())), Ok(failed.id()));
  let cancelled = cancelled(133);
  assert_eq!(
    run_again(&input(&[Some(&failed), Some(&cancelled)], cancelled.id())),
    Ok(cancelled.id())
  );
}

#[test]
fn run_again_is_not_offered_after_a_run_that_did_not_fail_or_was_cancelled() {
  let failed = failed(131);
  assert_eq!(run_again(&input(&[], failed.id())), Err(RunAgainBlock::NotEnded));
  // Only the latest run is run again, and only the one the person saw.
  let latest = cancelled(132);
  assert_eq!(
    run_again(&input(&[Some(&failed), Some(&latest)], failed.id())),
    Err(RunAgainBlock::NotEnded)
  );
  assert_eq!(
    run_again(&input(&[Some(&failed), None], failed.id())),
    Err(RunAgainBlock::NotEnded),
    "a latest run that cannot be read"
  );
}

#[test]
fn run_again_waits_for_another_active_run_of_the_task() {
  let failed = failed(131);
  let running = run(132).run;
  assert_eq!(
    run_again(&input(&[Some(&failed), Some(&running)], failed.id())),
    Err(RunAgainBlock::RunActive {
      number: running.number()
    })
  );
  let mut paused = run(130);
  paused.apply(|run| run.pause(at(12)));
  assert_eq!(
    run_again(&input(&[Some(&paused.run), Some(&failed)], failed.id())),
    Err(RunAgainBlock::RunActive {
      number: paused.run.number()
    })
  );
}

fn interrupted(number: u32) -> Run {
  let mut logged = run(number);
  logged.apply(|run| run.interrupt(InterruptCause::QuitNow, at(12)));
  logged.run
}

#[test]
fn run_again_follows_an_interrupted_run_only_once_its_worktree_was_removed() {
  let interrupted = interrupted(141);
  let runs = [Some(&interrupted)];
  assert_eq!(
    run_again(&input(&runs, interrupted.id())),
    Err(RunAgainBlock::RunActive {
      number: interrupted.number()
    }),
    "an interrupted run can still continue in its worktree"
  );
  assert!(holds_task(&interrupted, false));
  assert!(!can_run_again_after(&interrupted, false));

  let removed = [interrupted.id()];
  let without_worktree = RunAgainInput {
    worktree_removed: &removed,
    ..input(&runs, interrupted.id())
  };
  assert_eq!(run_again(&without_worktree), Ok(interrupted.id()));
  assert!(!holds_task(&interrupted, true));
  assert!(can_run_again_after(&interrupted, true));
}

#[test]
fn an_interrupted_run_without_its_worktree_no_longer_holds_its_task() {
  let interrupted = interrupted(140);
  let failed = failed(141);
  let runs = [Some(&interrupted), Some(&failed)];
  let removed = [interrupted.id()];
  assert_eq!(
    run_again(&input(&runs, failed.id())),
    Err(RunAgainBlock::RunActive {
      number: interrupted.number()
    })
  );
  let without_worktree = RunAgainInput {
    worktree_removed: &removed,
    ..input(&runs, failed.id())
  };
  assert_eq!(run_again(&without_worktree), Ok(failed.id()));
  // A removed worktree changes nothing for a run that is at work or ended.
  let running = run(142).run;
  assert!(holds_task(&running, true));
  assert!(!can_run_again_after(&running, true));
  assert!(can_run_again_after(&failed, true));
}

#[test]
fn run_again_needs_a_published_pipeline() {
  let failed = failed(131);
  let runs = [Some(&failed)];
  let unpublished = RunAgainInput {
    pipeline_published: false,
    ..input(&runs, failed.id())
  };
  assert_eq!(run_again(&unpublished), Err(RunAgainBlock::PipelineNotPublished));
}

#[test]
fn run_again_waits_while_the_daily_budget_is_used() {
  let failed = failed(131);
  let runs = [Some(&failed)];
  let limits = ProjectLimits::new(
    tokens(10_000_000),
    tokens(1_400_000),
    tokens(20_000_000),
    Duration::from_secs(1800),
  )
  .expect("limits");
  let used = RunAgainInput {
    limits,
    used_today: TokensUsed::new(20_000_000),
    ..input(&runs, failed.id())
  };
  assert_eq!(
    run_again(&used),
    Err(RunAgainBlock::DailyBudgetUsed {
      used: TokensUsed::new(20_000_000),
      budget: tokens(20_000_000),
    })
  );
  let room = RunAgainInput {
    used_today: TokensUsed::new(19_999_999),
    ..used
  };
  assert_eq!(run_again(&room), Ok(failed.id()));
}

#[test]
fn run_again_numbers_the_next_run_of_the_task_on_from_its_last() {
  let task = task_key(131);
  assert_eq!(BranchName::for_next_run(&task, 0).as_str(), "orchester/orc-131-run-1");
  assert_eq!(BranchName::for_next_run(&task, 1).as_str(), "orchester/orc-131-run-2");
}
