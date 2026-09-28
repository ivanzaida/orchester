//! Run lifecycle and recovery end to end (`docs/architecture/storage.md`, "Lifecycle"): a runner
//! killed mid-run, the next runner's attach for a reconnect or a new session, stopping at
//! checkpoints, the work summary and the daily project budget. Against `orchester-fake-codex`;
//! needs `git`. Run with `cargo test -p orchester-runner --features test-agent --test lifecycle`.

mod support;

use std::{num::NonZeroU32, path::Path, time::Duration};

use orchester_core::{
  connection::{AgentProvider, stream::InvocationOutcome},
  id::{IdempotencyKey, RunId, TaskId},
  limits::ProjectLimits,
  run::{ContinueTrigger, InterruptCause, PauseReason, RunEventKind, RunLimit, RunStatus},
  store::{ActivityStore, RunStore, SettingsStore},
  tokens::TokenBudget,
};
use orchester_ipc::{AttachSession, Event, Rejection, SessionEnd, WorkState, WorkSummary};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, killed_mid_run, project, reported, scheduler, wait_for, wait_for_turns};

fn slots(count: u32) -> NonZeroU32 {
  NonZeroU32::new(count).expect("above zero")
}

fn fresh(previous_end: SessionEnd) -> AttachSession {
  AttachSession::Fresh { previous_end }
}

fn start(scheduler: &Scheduler, task: TaskId) -> RunId {
  scheduler
    .start_run(IdempotencyKey::generate(), task)
    .expect("the run starts")
}

/// Waits for a work summary that satisfies `done`.
async fn summary_where(
  events: &mut mpsc::UnboundedReceiver<Event>,
  done: impl Fn(&WorkSummary) -> bool,
) -> WorkSummary {
  let wait = async {
    loop {
      if let Some(Event::WorkSummary(summary)) = events.recv().await
        && done(&summary)
      {
        return summary;
      }
    }
  };
  tokio::time::timeout(PATIENCE, wait)
    .await
    .expect("the summary arrives in time")
}

fn kinds(root: &Path, run: RunId) -> Vec<RunEventKind> {
  ProjectDb::open(root)
    .expect("open")
    .events(run)
    .expect("events")
    .into_iter()
    .map(|event| event.kind().clone())
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_reconnect_after_a_killed_runner_continues_from_the_checkpoint() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let run = killed_mid_run(root, project.task).await;
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.run(run).expect("read").expect("run").status(), &RunStatus::Running);

  let (scheduler, mut events) = scheduler(root);
  let attached = scheduler.attach(AttachSession::Reconnect, slots(2)).expect("attached");
  assert_eq!(attached.interrupted, vec![run]);
  assert_eq!(attached.continued, vec![run]);
  let done = wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);

  let kinds = kinds(root, run);
  assert!(kinds.contains(&RunEventKind::Interrupted {
    cause: InterruptCause::RunnerLost
  }));
  assert!(kinds.iter().any(|kind| matches!(
    kind,
    RunEventKind::Continued {
      trigger: ContinueTrigger::RunnerReconnected,
      ..
    }
  )));
  let invocations = db.invocations(run).expect("invocations");
  // The killed agent's invocation is closed, and the continue resumed its session.
  assert_eq!(invocations[0].outcome, Some(InvocationOutcome::Terminated));
  assert!(invocations[0].ended_at.is_some());
  let argv = reported(&db, run, "argv: ");
  let resumed = argv.last().expect("two launches");
  assert!(
    resumed.contains("exec resume") && resumed.contains("fake-thread-1"),
    "{argv:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_after_quit_now_runs_wait_for_continue_runs() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let run = killed_mid_run(root, project.task).await;

  let (scheduler, mut events) = scheduler(root);
  let attached = scheduler
    .attach(fresh(SessionEnd::QuitNow), slots(2))
    .expect("attached");
  assert_eq!(attached.interrupted, vec![run]);
  assert!(attached.continued.is_empty());
  let db = ProjectDb::open(root).expect("open");
  let waiting = db.run(run).expect("read").expect("run");
  assert_eq!(waiting.status(), &RunStatus::Interrupted(InterruptCause::QuitNow));
  assert!(scheduler.active_runs().is_empty());
  assert_eq!(db.interrupted_runs().expect("interrupted").len(), 1);

  let (continued, refused) = scheduler.continue_runs(&[run, RunId::generate()]);
  assert_eq!(continued, vec![run]);
  assert_eq!(refused.len(), 1);
  assert_eq!(refused[0].rejection, Rejection::RunNotFound);
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;
  assert!(kinds(root, run).iter().any(|kind| matches!(
    kind,
    RunEventKind::Continued {
      trigger: ContinueTrigger::User,
      ..
    }
  )));
  // A run that is not interrupted is not continued.
  let (continued, refused) = scheduler.continue_runs(&[run]);
  assert!(continued.is_empty());
  assert_eq!(refused[0].rejection, Rejection::RunNotInterrupted);
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_a_crash_interrupts_as_crashed() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let run = killed_mid_run(root, project.task).await;
  let (scheduler, _events) = scheduler(root);
  scheduler
    .attach(fresh(SessionEnd::Crashed), slots(2))
    .expect("attached");
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    db.run(run).expect("read").expect("run").status(),
    &RunStatus::Interrupted(InterruptCause::Crashed)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_work_summary_lists_the_working_run() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  scheduler.attach(fresh(SessionEnd::Clean), slots(3)).expect("attached");
  let empty = summary_where(&mut events, |_| true).await;
  assert_eq!(empty.agent_slots, slots(3));
  assert!(empty.runs.is_empty());

  let run = start(&scheduler, project.task);
  let working = summary_where(&mut events, |summary| !summary.runs.is_empty()).await;
  assert_eq!(working.agents_working(), 1);
  let work = &working.runs[0];
  assert_eq!(work.run, run);
  assert_eq!(work.task_key, "ORC-1");
  assert_eq!(work.task_title, "Say hello");
  assert_eq!(work.step, "Implement");
  assert_eq!(work.agent, "Builder");
  assert_eq!(work.provider, AgentProvider::CodexCli);
  assert_eq!(work.model, "gpt-5.5");
  assert_eq!(work.state, WorkState::Working);

  tokio::time::timeout(PATIENCE, scheduler.cancel_run(run))
    .await
    .expect("cancelled in time")
    .expect("cancelled");
  let gone = summary_where(&mut events, |summary| summary.runs.is_empty()).await;
  assert_eq!(gone.agents_working(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_agent_slots_hold_back_new_work() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let (scheduler, _events) = scheduler(root);
  scheduler.attach(fresh(SessionEnd::Clean), slots(1)).expect("attached");
  let other = support::queue_another_task(root);
  let run = start(&scheduler, project.task);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), other),
    Err(Rejection::NoAgentSlot { slots: slots(1) })
  );
  scheduler.stop_all(InterruptCause::QuitNow).await;
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    db.run(run).expect("read").expect("run").status(),
    &RunStatus::Interrupted(InterruptCause::QuitNow)
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_finish_at_checkpoints_settles_every_run() {
  let project = project(&Setup {
    review: true,
    ..Setup::new("Work slowly.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  scheduler.attach(fresh(SessionEnd::Clean), slots(2)).expect("attached");
  let run = start(&scheduler, project.task);
  wait_for_turns(root, &mut events, run, 1).await;

  let finishing = tokio::spawn({
    let scheduler = scheduler.clone();
    async move { scheduler.finish_at_checkpoints().await }
  });
  let saving = summary_where(&mut events, |summary| {
    summary.runs.iter().any(|work| work.state == WorkState::Saving)
  })
  .await;
  assert_eq!(saving.runs.len(), 1);
  let saved = summary_where(&mut events, |summary| {
    summary
      .runs
      .iter()
      .any(|work| matches!(work.state, WorkState::Saved { .. }))
  })
  .await;
  tokio::time::timeout(PATIENCE, finishing)
    .await
    .expect("settled in time")
    .expect("finishing ended");

  let db = ProjectDb::open(root).expect("open");
  let paused = db.run(run).expect("read").expect("run");
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::Requested));
  let checkpoint = paused.last_checkpoint().expect("Implement's work was committed");
  assert_eq!(
    saved.runs[0].state,
    WorkState::Saved {
      checkpoint: Some(checkpoint.revision().as_str().to_owned())
    }
  );
  assert_eq!(saved.agents_working(), 0);
  // Stopping starts no new work.
  assert_eq!(scheduler.resume_run(run).await, Err(Rejection::ShuttingDown));
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_daily_budget_stops_the_run_and_refuses_new_work() {
  let project = project(&Setup::new("Work in turns."));
  let root = project.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  let tight = ProjectLimits::new(
    TokenBudget::new(10_000_000).expect("above zero"),
    TokenBudget::new(1_400_000).expect("above zero"),
    TokenBudget::new(5_000).expect("above zero"),
    Duration::from_secs(30 * 60),
  )
  .expect("limits");
  db.save_project_limits(&tight).expect("save limits");
  let (scheduler, mut events) = scheduler(root);
  let run = start(&scheduler, project.task);

  // The first turn used 9,739 counted tokens, above today's 5,000: the next turn stops it.
  let stopped = wait_for(root, &mut events, run, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(
    stopped.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::DailyBudget))
  );
  assert!(
    stopped.last_checkpoint().is_some(),
    "the work at the limit is committed"
  );
  let daily = Rejection::DailyBudgetUsed {
    budget: tight.daily_budget(),
  };
  assert_eq!(scheduler.resume_run(run).await, Err(daily.clone()));
  let other = support::queue_another_task(root);
  assert_eq!(scheduler.start_run(IdempotencyKey::generate(), other), Err(daily));

  // A raised daily budget makes room: the run resumes as it is.
  db.save_project_limits(&ProjectLimits::DEFAULT).expect("save limits");
  assert_eq!(scheduler.resume_run(run).await, Ok(()));
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Running).await;
  scheduler.stop_all(InterruptCause::QuitNow).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_two_runs_of_one_project_both_settle_at_their_checkpoints() {
  let project = project(&Setup {
    review: true,
    ..Setup::new("Work slowly.")
  });
  let root = project.repo.path();
  let other = support::queue_another_task(root);
  let (scheduler, mut events) = scheduler(root);
  scheduler.attach(fresh(SessionEnd::Clean), slots(2)).expect("attached");
  let runs = [start(&scheduler, project.task), start(&scheduler, other)];
  for run in runs {
    wait_for_turns(root, &mut events, run, 1).await;
  }
  tokio::time::timeout(PATIENCE, scheduler.finish_at_checkpoints())
    .await
    .expect("both settle in time");
  let db = ProjectDb::open(root).expect("open");
  for run in runs {
    let run = db.run(run).expect("read").expect("run");
    assert_eq!(run.status(), &RunStatus::Paused(PauseReason::Requested));
    assert!(run.last_checkpoint().is_some(), "each run's work is committed");
  }
}

/// The runner has work while a run executes and none once it settled: only then may a runner no
/// tab shows stop.
#[tokio::test(flavor = "multi_thread")]
async fn lifecycle_scheduler_is_idle_only_while_no_run_executes() {
  let project = project(&Setup::new("Work slowly."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  scheduler
    .attach(fresh(SessionEnd::Clean), slots(1))
    .expect("attached");
  assert!(scheduler.is_idle(), "nothing runs yet");

  let run = start(&scheduler, project.task);
  assert!(!scheduler.is_idle(), "a run executes");
  wait_for(root, &mut events, run, |run| run.status() == &RunStatus::Succeeded).await;
  let settled = async {
    while !scheduler.is_idle() {
      tokio::time::sleep(Duration::from_millis(20)).await;
    }
  };
  tokio::time::timeout(PATIENCE, settled)
    .await
    .expect("idle once the run ended");
}
