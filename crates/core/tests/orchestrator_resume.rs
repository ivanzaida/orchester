//! What the Orchestrator chat's `resume_run` may do with a run (`docs/design/orchestrator-toolset.md`, decision 5): a
//! pause is resumed; a crash, the runner going away and *Quit now* are continued under the person's switch; every
//! limit, a person's cancel, the runner failing to record the run and a gone worktree stay the person's; and a run
//! interrupted again within ten minutes of the time before is left to the person.
//! `cargo test -p orchester-core --test orchestrator_resume resume`.

use orchester_core::{
  id::StepExecutionId,
  orchestrator::{ChatResume, Continuing, PersonsStop, resume::AGAIN_WITHIN},
  resource::StepKey,
  run::{InterruptCause, PauseReason, RunFailure, RunLimit, RunStatus, Stuck},
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
      ChatResume::of(&RunStatus::Interrupted(cause), true),
      ChatResume::Interrupted(cause),
      "{cause:?}"
    );
  }
  assert_eq!(
    ChatResume::of(&RunStatus::Paused(PauseReason::Requested), true),
    ChatResume::Paused
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
      ChatResume::of(&RunStatus::Paused(PauseReason::LimitReached(limit)), true),
      ChatResume::PersonOnly(PersonsStop::Limit(limit)),
      "{limit:?}"
    );
  }
  assert_eq!(
    ChatResume::of(&RunStatus::Cancelled, true),
    ChatResume::PersonOnly(PersonsStop::Canceled)
  );
  assert_eq!(
    ChatResume::of(&RunStatus::Interrupted(InterruptCause::RunnerFailed), true),
    ChatResume::PersonOnly(PersonsStop::RunnerFailed)
  );
  for cause in [
    InterruptCause::Crashed,
    InterruptCause::RunnerLost,
    InterruptCause::QuitNow,
    InterruptCause::RunnerFailed,
  ] {
    assert_eq!(
      ChatResume::of(&RunStatus::Interrupted(cause), false),
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
    assert_eq!(ChatResume::of(&status, true), ChatResume::Other, "{status:?}");
  }
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
