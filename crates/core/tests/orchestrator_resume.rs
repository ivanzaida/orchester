//! What the Orchestrator chat's `resume_run` may do with a run (`docs/design/orchestrator-toolset.md`, decision 5): a
//! pause is resumed; a crash, the runner going away, *Quit now* and the database staying busy (ORC-75) are continued
//! under the person's switch; every limit, a person's cancel, the runner failing to record the run for another reason
//! or one it did not record, and a gone worktree stay the person's; a run interrupted again within ten minutes of the
//! time before is left to the person, and so is one the database staying busy interrupted while more than three runs
//! of the project were so within ten minutes.
//! `cargo test -p orchester-core --test orchestrator_resume resume`.

use orchester_core::{
  id::StepExecutionId,
  orchestrator::{
    ChatResume, Continuing, PersonsStop,
    resume::{AGAIN_WITHIN, CONTENTION_BRAKE},
  },
  resource::StepKey,
  run::{InterruptCause, PauseReason, RunFailure, RunLimit, RunStatus, RunnerFailure, Stuck},
  time::Timestamp,
};

#[test]
fn resume_continues_a_crash_the_runner_going_away_and_quit_now() {
  for cause in [
    InterruptCause::Crashed,
    InterruptCause::RunnerLost,
    InterruptCause::QuitNow,
  ] {
    assert_eq!(
      ChatResume::of(&RunStatus::Interrupted(cause), None, true),
      ChatResume::Interrupted(cause),
      "{cause:?}"
    );
  }
  assert_eq!(
    ChatResume::of(&RunStatus::Paused(PauseReason::Requested), None, true),
    ChatResume::Paused
  );
}

#[test]
fn resume_continues_a_runner_failure_only_of_storage_contention() {
  let failed = RunStatus::Interrupted(InterruptCause::RunnerFailed);
  assert_eq!(
    ChatResume::of(&failed, Some(RunnerFailure::StorageContention), true),
    ChatResume::Interrupted(InterruptCause::RunnerFailed),
    "the database stayed busy: transient"
  );
  for failure in [Some(RunnerFailure::StorageFailure), Some(RunnerFailure::Other), None] {
    assert_eq!(
      ChatResume::of(&failed, failure, true),
      ChatResume::PersonOnly(PersonsStop::RunnerFailed),
      "{failure:?}: the person's (None: an old run whose kind was not recorded)"
    );
  }
  assert_eq!(
    ChatResume::of(&failed, Some(RunnerFailure::StorageContention), false),
    ChatResume::PersonOnly(PersonsStop::WorktreeGone),
    "contention without its worktree"
  );
}

#[test]
fn resume_leaves_every_limit_a_cancel_a_runner_failure_and_a_gone_worktree_to_the_person() {
  let execution = StepExecutionId::generate();
  for limit in [
    RunLimit::Tokens,
    RunLimit::Time,
    RunLimit::DailyBudget,
    RunLimit::GroupBudget,
    RunLimit::StepBudget { execution },
    RunLimit::PlanLimit { execution },
  ] {
    assert_eq!(
      ChatResume::of(&RunStatus::Paused(PauseReason::LimitReached(limit)), None, true),
      ChatResume::PersonOnly(PersonsStop::Limit(limit)),
      "{limit:?}"
    );
  }
  assert_eq!(
    ChatResume::of(&RunStatus::Cancelled, None, true),
    ChatResume::PersonOnly(PersonsStop::Canceled)
  );
  assert_eq!(
    ChatResume::of(&RunStatus::Interrupted(InterruptCause::RunnerFailed), None, true),
    ChatResume::PersonOnly(PersonsStop::RunnerFailed)
  );
  for cause in [
    InterruptCause::Crashed,
    InterruptCause::RunnerLost,
    InterruptCause::QuitNow,
    InterruptCause::RunnerFailed,
  ] {
    assert_eq!(
      ChatResume::of(&RunStatus::Interrupted(cause), None, false),
      ChatResume::PersonOnly(PersonsStop::WorktreeGone),
      "{cause:?} without its worktree"
    );
  }
}

#[test]
fn resume_leaves_what_did_not_stop_so_to_the_tool() {
  let step = StepKey::new("implement").expect("key");
  for status in [
    RunStatus::Pending,
    RunStatus::Running,
    RunStatus::WaitingForPerson(Stuck::BrokeAgain(step.clone())),
    RunStatus::Succeeded,
    RunStatus::Failed(RunFailure::StepFailed(step)),
  ] {
    assert_eq!(ChatResume::of(&status, None, true), ChatResume::Other, "{status:?}");
  }
}

#[test]
fn resume_of_contention_stops_while_more_than_three_runs_were_so() {
  let window = i64::try_from(AGAIN_WITHIN.as_millis()).expect("fits");
  let at = Timestamp::from_unix_millis;
  assert_eq!(CONTENTION_BRAKE, 3);
  for contended in 1..=CONTENTION_BRAKE {
    assert_eq!(
      Continuing::after_contention(&[at(1_000)], contended),
      Continuing::Continues,
      "{contended} runs: continued once"
    );
  }
  assert_eq!(
    Continuing::after_contention(&[at(1_000)], CONTENTION_BRAKE + 1),
    Continuing::StorageContention,
    "the brake holds"
  );
  assert_eq!(
    Continuing::after_contention(&[at(1_000), at(1_000 + window)], 1),
    Continuing::InterruptedAgain,
    "a repeat within ten minutes is the person's"
  );
}

#[test]
fn resume_of_a_run_interrupted_again_soon_is_left_to_the_person() {
  let window = i64::try_from(AGAIN_WITHIN.as_millis()).expect("fits");
  let at = Timestamp::from_unix_millis;
  assert_eq!(
    Continuing::after(&[at(1_000)]),
    Continuing::Continues,
    "the first interruption"
  );
  assert_eq!(
    Continuing::after(&[at(1_000), at(1_000 + window)]),
    Continuing::InterruptedAgain,
    "twice within ten minutes"
  );
  assert_eq!(
    Continuing::after(&[at(1_000), at(1_000 + window + 1)]),
    Continuing::Continues,
    "the time before is long ago"
  );
  assert_eq!(
    Continuing::after(&[at(0), at(5), at(10 + 2 * window)]),
    Continuing::Continues,
    "only the last two count"
  );
  assert_eq!(AGAIN_WITHIN.as_secs(), 600);
}
