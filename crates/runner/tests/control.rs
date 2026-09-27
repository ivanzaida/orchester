//! Pausing, resuming and cancelling runs, and their token and time limits, end to end against
//! `orchester-fake-codex` (see `tests/support/fake_codex.rs`). Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test control`.

mod support;

use std::time::Duration;

use orchester_core::{
  connection::stream::InvocationOutcome,
  id::{IdempotencyKey, RunId},
  resource::StepKey,
  run::{PauseReason, RaisedLimits, Run, RunLimit, RunStatus, StepStatus},
  store::{ActivityStore, RunStore},
  tokens::{TokenBudget, TokensUsed},
};
use orchester_ipc::{Event, Rejection};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use tokio::sync::mpsc;

use self::support::{PATIENCE, Setup, git, is_running, project, reported, scheduler, wait_for, wait_for_turns};

fn step_status(run: &Run, step: &str) -> Option<StepStatus> {
  run
    .latest_execution(&StepKey::new(step).expect("key"))
    .map(|execution| execution.status())
}

fn raised(budget: u64, max_duration: Duration) -> RaisedLimits {
  RaisedLimits {
    budget: TokenBudget::new(budget).expect("above zero"),
    max_duration,
  }
}

async fn start(scheduler: &Scheduler, task: orchester_core::id::TaskId) -> RunId {
  scheduler
    .start_run(IdempotencyKey::generate(), task)
    .expect("the run starts")
}

/// The file at the head of the run's branch.
fn readme_at(root: &std::path::Path, revision: &str) -> String {
  git(root, &["show", &format!("{revision}:README.md")])
}

#[tokio::test(flavor = "multi_thread")]
async fn control_pause_waits_for_the_checkpoint_and_resume_continues() {
  let project = project(&Setup {
    review: true,
    ..Setup::new("Work slowly.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = start(&scheduler, project.task).await;
  wait_for_turns(root, &mut events, id, 1).await;
  tokio::time::timeout(PATIENCE, scheduler.pause_run(id))
    .await
    .expect("answered in time")
    .expect("the pause is requested");
  let db = ProjectDb::open(root).expect("open");
  let pausing = db.run(id).expect("read").expect("run");
  // Pausing is not paused: the agent finishes its invocation first.
  assert!(pausing.pause_requested());
  assert_eq!(pausing.status(), &RunStatus::Running);

  let paused = wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(paused.status(), &RunStatus::Paused(PauseReason::Requested));
  assert_eq!(step_status(&paused, "implement"), Some(StepStatus::Passed));
  assert_eq!(step_status(&paused, "review"), Some(StepStatus::Pending));
  let checkpoint = paused.last_checkpoint().expect("the step's work was committed");
  assert_eq!(readme_at(root, checkpoint.revision().as_str()), "hi\nhello");
  assert!(scheduler.active_runs().is_empty());
  // Pausing a paused run changes nothing.
  assert_eq!(scheduler.pause_run(id).await, Ok(()));

  scheduler.resume_run(id).await.expect("the run resumes");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  let argv = reported(&db, id, "argv: ");
  assert_eq!(argv.len(), 2);
  assert!(argv.iter().all(|argv| !argv.contains("resume")), "{argv:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn control_cancel_ends_the_agent_and_its_processes_and_keeps_the_worktree() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = start(&scheduler, project.task).await;
  wait_for_turns(root, &mut events, id, 1).await;
  let db = ProjectDb::open(root).expect("open");
  let helper = tokio::time::timeout(PATIENCE, async {
    loop {
      if let Some(helper) = reported(&db, id, "helper pid: ").pop() {
        return helper;
      }
      events.recv().await.expect("the scheduler notifies");
    }
  })
  .await
  .expect("the helper starts in time");
  let agent = reported(&db, id, "pid: ").pop().expect("the agent's pid");
  assert!(is_running(&agent) && is_running(&helper));

  tokio::time::timeout(PATIENCE, scheduler.cancel_run(id))
    .await
    .expect("answered in time")
    .expect("the run is cancelled");
  let run = db.run(id).expect("read").expect("run");
  assert_eq!(run.status(), &RunStatus::Cancelled);
  assert_eq!(step_status(&run, "implement"), Some(StepStatus::Skipped));
  let invocation = &db.invocations(id).expect("invocations")[0];
  assert_eq!(invocation.outcome, Some(InvocationOutcome::Terminated));
  assert!(!is_running(&agent));
  // Ending a process tree is a known gap on macOS (`AGENTS.md`); the helper outlives the agent there.
  if cfg!(windows) {
    assert!(!is_running(&helper));
  }
  assert!(root.join(".orchester").join("worktrees").join(id.to_string()).is_dir());
  assert!(!git(root, &["rev-parse", "--verify", "orchester/orc-1-run-1"]).is_empty());
  assert_eq!(scheduler.cancel_run(id).await, Err(Rejection::RunEnded));
  assert_eq!(scheduler.pause_run(id).await, Err(Rejection::RunNotRunning));
}

#[tokio::test(flavor = "multi_thread")]
async fn control_token_limit_commits_the_work_and_continues_the_session_once_raised() {
  let project = project(&Setup {
    budget: 5_000,
    ..Setup::new("Work in turns.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = start(&scheduler, project.task).await;
  let stopped = wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(
    stopped.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::Tokens))
  );
  assert_eq!(stopped.used(), TokensUsed::new(12_166 - 2_432 + 5));
  assert_eq!(step_status(&stopped, "implement"), Some(StepStatus::Running));
  let checkpoint = stopped.last_checkpoint().expect("the work at the limit was committed");
  assert_eq!(readme_at(root, checkpoint.revision().as_str()), "hi\ndraft");
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(
    db.invocations(id).expect("invocations")[0].outcome,
    Some(InvocationOutcome::Terminated)
  );

  let long = Duration::from_secs(30 * 60);
  assert_eq!(
    scheduler.resume_run(id).await,
    Err(Rejection::LimitNotRaised {
      limit: RunLimit::Tokens
    })
  );
  assert_eq!(
    scheduler.raise_limit(id, raised(9_000, long)).await,
    Err(Rejection::LimitNotAboveUsed {
      limit: RunLimit::Tokens
    })
  );
  assert_eq!(
    scheduler.raise_limit(id, raised(12_000_000, long)).await,
    Err(Rejection::BudgetAboveMaximum {
      max: TokenBudget::new(10_000_000).expect("above zero")
    })
  );
  scheduler
    .raise_limit(id, raised(2_000_000, long))
    .await
    .expect("the run continues");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  assert_eq!(done.budget().get(), 2_000_000);
  let head = done
    .last_checkpoint()
    .expect("checkpoint")
    .revision()
    .as_str()
    .to_owned();
  assert_eq!(readme_at(root, &head), "hi\ndraft\ncontinued");
  let argv = reported(&db, id, "argv: ");
  assert_eq!(argv.len(), 2);
  assert!(argv[1].starts_with("-a never exec resume --json"), "{}", argv[1]);
  assert!(argv[1].contains("sandbox_mode=workspace-write"), "{}", argv[1]);
  assert!(argv[1].ends_with("fake-thread-1 -"), "{}", argv[1]);
  let invocations = db.invocations(id).expect("invocations");
  assert_eq!(invocations[1].session.as_deref(), Some("fake-thread-1"));
  assert_eq!(invocations[1].number, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn control_time_limit_counts_active_time_and_stops_the_agent() {
  let project = project(&Setup {
    max_duration: Duration::from_secs(2),
    ..Setup::new("Keep working.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let id = start(&scheduler, project.task).await;
  let stopped = wait_for(root, &mut events, id, |run| {
    matches!(run.status(), RunStatus::Paused(_))
  })
  .await;
  assert_eq!(
    stopped.status(),
    &RunStatus::Paused(PauseReason::LimitReached(RunLimit::Time))
  );
  // Nothing changed in the worktree, so there was nothing to commit.
  assert!(stopped.last_checkpoint().is_none());
  let budget = 1_400_000;
  assert_eq!(
    scheduler.raise_limit(id, raised(budget, Duration::from_secs(1))).await,
    Err(Rejection::LimitNotAboveUsed { limit: RunLimit::Time })
  );
  // The time the run spent stopped does not count: the new limit is measured against the agent's time.
  tokio::time::sleep(Duration::from_secs(2)).await;
  scheduler
    .raise_limit(id, raised(budget, Duration::from_secs(60)))
    .await
    .expect("the run continues");
  let done = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(done.status(), &RunStatus::Succeeded);
  let db = ProjectDb::open(root).expect("open");
  let argv = reported(&db, id, "argv: ");
  assert!(argv[1].contains("exec resume"), "{}", argv[1]);
}

/// A run whose start failed stays pending. Cancelling it ends it without an attempt, and the
/// task's runs still read.
#[tokio::test(flavor = "multi_thread")]
async fn control_cancel_ends_a_pending_run_that_never_started() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  // The run's branch exists already, so its worktree cannot be created and the run never starts.
  git(root, &["branch", "orchester/orc-1-run-1"]);
  let (scheduler, mut events) = scheduler(root);
  assert!(scheduler.start_run(IdempotencyKey::generate(), project.task).is_err());
  let db = ProjectDb::open(root).expect("open");
  let [pending] = db
    .runs_of_task(project.task)
    .expect("runs")
    .try_into()
    .expect("one run");
  let pending = pending.expect("readable run");
  assert_eq!(pending.status(), &RunStatus::Pending);

  tokio::time::timeout(PATIENCE, scheduler.cancel_run(pending.id()))
    .await
    .expect("answered in time")
    .expect("the run is cancelled");
  let cancelled = db.run(pending.id()).expect("read").expect("run");
  assert_eq!(cancelled.status(), &RunStatus::Cancelled);
  assert!(cancelled.attempts().is_empty());
  let runs: Vec<_> = db
    .runs_of_task(project.task)
    .expect("runs")
    .into_iter()
    .map(|run| run.expect("readable run"))
    .collect();
  assert_eq!(runs, vec![cancelled]);
  let told = std::iter::from_fn(|| events.try_recv().ok())
    .any(|event| matches!(event, Event::RunChanged { run, .. } if run == pending.id()));
  assert!(told, "the desktop is told that the run changed");
  assert!(scheduler.active_runs().is_empty());
  assert_eq!(scheduler.cancel_run(pending.id()).await, Err(Rejection::RunEnded));
}

#[tokio::test(flavor = "multi_thread")]
async fn control_rejects_requests_for_unknown_and_running_runs() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let (scheduler, mut events): (Scheduler, mpsc::UnboundedReceiver<_>) = scheduler(root);
  let unknown = RunId::generate();
  assert_eq!(scheduler.pause_run(unknown).await, Err(Rejection::RunNotFound));
  assert_eq!(scheduler.resume_run(unknown).await, Err(Rejection::RunNotFound));
  assert_eq!(scheduler.cancel_run(unknown).await, Err(Rejection::RunNotFound));
  let id = start(&scheduler, project.task).await;
  wait_for_turns(root, &mut events, id, 1).await;
  assert_eq!(scheduler.resume_run(id).await, Err(Rejection::RunNotPaused));
  assert_eq!(
    scheduler
      .raise_limit(id, raised(2_000_000, Duration::from_secs(60)))
      .await,
    Err(Rejection::RunNotPaused)
  );
  scheduler.cancel_run(id).await.expect("cancel");
}
