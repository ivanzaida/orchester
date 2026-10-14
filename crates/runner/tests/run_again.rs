//! Run again end to end against `orchester-fake-codex`: after a failed or cancelled run the task
//! starts its next run from the first step in a new worktree on a new branch, once per request;
//! what blocks it; removing the worktree of the run before; and an interrupted run whose worktree
//! a person removed, which can no longer continue and no longer holds its task. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test run_again`.

mod support;

use std::{fs, path::Path, time::Duration};

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, ResourceId, RunId, TaskId},
  integration::{Leftovers, clean_up_recorded, leftovers},
  limits::ProjectLimits,
  name::Name,
  resource::{Pipeline, Slug, StepKind},
  run::{InterruptCause, Run, RunAgainBlock, RunStatus},
  store::{ResourceStore, RunStore, SettingsStore, TaskStore, WorktreeStore},
  time::Timestamp,
  tokens::{TokenBudget, TokensUsed},
};
use orchester_exec::ProcessRunner;
use orchester_ipc::Rejection;
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;
use rusqlite::Connection;

use self::support::{Setup, project, scheduler, wait_for, wait_for_turns};

async fn start(scheduler: &Scheduler, task: TaskId) -> RunId {
  scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts")
}

fn run(root: &Path, id: RunId) -> Run {
  ProjectDb::open(root)
    .expect("open")
    .run(id)
    .expect("read")
    .expect("run")
}

fn worktree(root: &Path, run: RunId) -> std::path::PathBuf {
  root.join(".orchester/worktrees").join(run.to_string())
}

fn left_by(root: &Path, id: RunId) -> Leftovers {
  let git = ProcessRunner::new(Duration::from_secs(30));
  leftovers(&git, Path::new("git"), root, &run(root, id)).expect("what the run left")
}

fn blocked(block: RunAgainBlock) -> Rejection {
  Rejection::RunAgainBlocked { block }
}

#[tokio::test(flavor = "multi_thread")]
async fn run_again_after_a_failed_run_starts_the_next_run_from_the_first_step() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let first = start(&scheduler, project.task).await;
  let failed = wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  assert!(matches!(failed.status(), RunStatus::Failed(_)));

  let mut db = ProjectDb::open(root).expect("open");
  let task = db.task(project.task).expect("read task").expect("task");
  let pipeline = task.details().pipeline.expect("pipeline");
  let v1 = db.latest_version::<Pipeline>(pipeline).expect("read v1").expect("v1");
  assert_eq!(task.pipeline_version(), Some(v1.number()));
  assert_eq!(failed.snapshot().pipeline().version, v1.id());
  let mut steps = v1.body().steps().to_vec();
  steps[0].name = Name::new("Implement revised").expect("step name");
  let now = Timestamp::now();
  let mut draft = v1.to_draft(now);
  draft.edit(Pipeline::new(steps).expect("pipeline"), now);
  db.save_draft(pipeline, &draft).expect("save draft");
  let v2 = db.publish_draft::<Pipeline>(pipeline, now).expect("publish v2");
  drop(db);

  let again = scheduler
    .run_again(IdempotencyKey::generate(), project.task, first, None)
    .await
    .expect("the task runs again");
  assert_ne!(again, first);
  let next = run(root, again);
  assert_eq!(next.task(), project.task);
  assert_eq!(next.snapshot().pipeline().version, v2.id());
  assert_eq!(next.snapshot().pipeline().number, v2.number());
  assert_eq!(next.snapshot().body().steps()[0].name.as_str(), "Implement revised");
  assert_eq!(next.number().get(), failed.number().get() + 1);
  assert_eq!(next.branch().as_str(), "orchester/orc-1-run-2");
  assert_ne!(next.status(), &RunStatus::Pending, "the run started");
  assert!(worktree(root, again).is_dir(), "a new worktree");
  assert!(worktree(root, first).is_dir(), "the run before keeps its worktree");
  assert_eq!(run(root, first), failed, "the run before keeps its record");
  let ended = wait_for(root, &mut events, again, |run| run.status().is_terminal()).await;
  let implement = ended.attempts()[0].executions()[0].step().as_str().to_owned();
  assert_eq!(implement, "implement", "from the first step");
}

#[tokio::test(flavor = "multi_thread")]
async fn run_again_after_a_cancelled_run_starts_once_per_request() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let first = start(&scheduler, project.task).await;
  wait_for_turns(root, &mut events, first, 1).await;
  scheduler.cancel_run(first).await.expect("the run is cancelled");

  // The confirmation pressed twice, and its reply lost and retried: one run.
  let key = IdempotencyKey::generate();
  let (once, twice) = tokio::join!(
    scheduler.run_again(key, project.task, first, None),
    scheduler.run_again(key, project.task, first, None),
  );
  let again = once.expect("the task runs again");
  assert_eq!(twice, Ok(again));
  assert_eq!(scheduler.run_again(key, project.task, first, None).await, Ok(again));
  let mut db = ProjectDb::open(root).expect("open");
  let task = db.task(project.task).expect("read task").expect("task");
  let pipeline = task.details().pipeline.expect("pipeline");
  let v1 = db.latest_version::<Pipeline>(pipeline).expect("read v1").expect("v1");
  let mut steps = v1.body().steps().to_vec();
  let StepKind::Agent { profile, .. } = &mut steps[0].kind else {
    panic!("the first step is an agent");
  };
  *profile = Some(ResourceId::generate());
  let now = Timestamp::now();
  let mut draft = v1.to_draft(now);
  draft.edit(Pipeline::new(steps).expect("pipeline"), now);
  db.save_draft(pipeline, &draft).expect("save draft");
  db.publish_draft::<Pipeline>(pipeline, now)
    .expect("publish invalid-for-run v2");
  drop(db);
  assert_eq!(
    scheduler.run_again(key, project.task, first, None).await,
    Ok(again),
    "a recorded request does not recheck a later pipeline version"
  );
  let runs = ProjectDb::open(root)
    .expect("open")
    .runs_of_task(project.task)
    .expect("runs");
  assert_eq!(runs.len(), 2);
  // Another confirmation for the same run finds the new one active.
  let number = run(root, again).number();
  assert_eq!(
    scheduler
      .run_again(IdempotencyKey::generate(), project.task, first, None)
      .await,
    Err(blocked(RunAgainBlock::RunActive { number }))
  );
  scheduler.stop_all(InterruptCause::QuitNow).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn run_again_is_refused_for_a_run_that_has_not_ended() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let first = start(&scheduler, project.task).await;
  wait_for_turns(root, &mut events, first, 1).await;
  assert_eq!(
    scheduler
      .run_again(IdempotencyKey::generate(), project.task, first, None)
      .await,
    Err(blocked(RunAgainBlock::RunActive {
      number: run(root, first).number()
    }))
  );
  scheduler.stop_all(InterruptCause::QuitNow).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn run_again_needs_a_published_pipeline() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let first = start(&scheduler, project.task).await;
  wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  // The task names a pipeline that was never published, as an edit outside Orchester could.
  let mut db = ProjectDb::open(root).expect("open");
  let task = db.task(project.task).expect("read").expect("task");
  let published = task.details().pipeline.expect("a pipeline");
  let body = db
    .latest_version::<Pipeline>(published)
    .expect("read")
    .expect("published")
    .body()
    .clone();
  let draft = db
    .create_resource(
      Slug::new("draft").expect("slug"),
      Name::new("Draft").expect("name"),
      &body,
      Timestamp::now(),
    )
    .expect("draft pipeline")
    .id();
  Connection::open(root.join(".orchester/orchester.db"))
    .expect("open raw database")
    .execute(
      "UPDATE tasks SET pipeline_id = ?1 WHERE id = ?2",
      [draft.to_string(), project.task.to_string()],
    )
    .expect("point the task at the draft");
  assert_eq!(
    scheduler
      .run_again(IdempotencyKey::generate(), project.task, first, None)
      .await,
    Err(blocked(RunAgainBlock::PipelineNotPublished))
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn run_again_waits_while_the_daily_budget_is_used() {
  let project = project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new("Work in turns.")
  });
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let first = start(&scheduler, project.task).await;
  wait_for_turns(root, &mut events, first, 2).await;
  scheduler.cancel_run(first).await.expect("the run is cancelled");
  let mut db = ProjectDb::open(root).expect("open");
  let budget = TokenBudget::new(5_000).expect("above zero");
  let tight = ProjectLimits::new(
    Some(TokenBudget::new(1_400_000).expect("above zero")),
    budget,
    Some(Duration::from_secs(30 * 60)),
  )
  .expect("limits");
  db.save_project_limits(&tight).expect("save limits");
  let refused = scheduler
    .run_again(IdempotencyKey::generate(), project.task, first, None)
    .await;
  let Err(Rejection::RunAgainBlocked {
    block: RunAgainBlock::DailyBudgetUsed { used, budget: daily },
  }) = refused
  else {
    panic!("refused for the daily budget: {refused:?}");
  };
  assert_eq!(daily, budget);
  assert!(used >= TokensUsed::new(5_000), "{used:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn run_again_removes_the_worktree_of_the_run_before_as_confirmed() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let first = start(&scheduler, project.task).await;
  wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  let old = worktree(root, first);
  fs::write(old.join("notes.txt"), "not committed\n").expect("an uncommitted file");
  let seen = left_by(root, first);
  assert_eq!(seen.uncommitted(), 1);

  // A file that appeared after the person saw the worktree keeps it, and nothing starts.
  fs::write(old.join("later.txt"), "later\n").expect("a later file");
  let refused = scheduler
    .run_again(IdempotencyKey::generate(), project.task, first, Some(seen))
    .await;
  assert!(
    matches!(&refused, Err(Rejection::WorktreeChanged { path }) if path.ends_with(&first.to_string())),
    "{refused:?}"
  );
  assert!(old.is_dir());
  let db = ProjectDb::open(root).expect("open");
  assert_eq!(db.runs_of_task(project.task).expect("runs").len(), 1);

  let seen = left_by(root, first);
  let again = scheduler
    .run_again(IdempotencyKey::generate(), project.task, first, Some(seen))
    .await
    .expect("the task runs again");
  assert!(!old.exists(), "the worktree of the run before was removed");
  assert!(worktree(root, again).is_dir());
  assert!(
    db.run(first).expect("read").is_some(),
    "the run before keeps its record"
  );
  assert!(
    db.worktree_removal(first).expect("read").is_some(),
    "the removal is recorded"
  );
  wait_for(root, &mut events, again, |run| run.status().is_terminal()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn run_again_follows_an_interrupted_run_once_its_worktree_was_removed() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let (stopping, mut events) = scheduler(root);
  let first = start(&stopping, project.task).await;
  wait_for_turns(root, &mut events, first, 1).await;
  stopping.stop_all(InterruptCause::QuitNow).await;
  let interrupted = run(root, first);
  assert_eq!(interrupted.status(), &RunStatus::Interrupted(InterruptCause::QuitNow));
  let (scheduler, mut events) = scheduler(root);
  assert_eq!(
    scheduler
      .run_again(IdempotencyKey::generate(), project.task, first, None)
      .await,
    Err(blocked(RunAgainBlock::RunActive {
      number: interrupted.number()
    })),
    "with its worktree it can still continue"
  );

  // Remove worktree, as the task page does it: the database records it, then git removes it.
  let git = ProcessRunner::new(Duration::from_secs(60));
  let seen = left_by(root, first);
  let mut db = ProjectDb::open(root).expect("open");
  let cleanup = clean_up_recorded(
    &mut db,
    (&git, Path::new("git"), root),
    &interrupted,
    Some(&seen),
    Timestamp::now(),
  )
  .expect("removed");
  assert!(cleanup.removed_run_worktree());
  assert!(db.worktree_removal(first).expect("read").is_some());

  let (continued, refused) = scheduler.continue_runs(&[first]).await;
  assert!(continued.is_empty());
  assert_eq!(refused[0].rejection, Rejection::WorktreeRemoved);
  assert_eq!(run(root, first), interrupted, "it stays interrupted, unchanged");

  let again = scheduler
    .run_again(IdempotencyKey::generate(), project.task, first, None)
    .await
    .expect("the task runs again");
  assert_eq!(run(root, again).number().get(), interrupted.number().get() + 1);
  assert!(worktree(root, again).is_dir());
  assert_eq!(
    run(root, first).status(),
    &RunStatus::Interrupted(InterruptCause::QuitNow)
  );
  wait_for_turns(root, &mut events, again, 1).await;
  scheduler.stop_all(InterruptCause::QuitNow).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn run_again_follows_an_interrupted_run_whose_worktree_was_deleted_by_hand() {
  let project = project(&Setup::new("Keep working."));
  let root = project.repo.path();
  let (stopping, mut events) = scheduler(root);
  let first = start(&stopping, project.task).await;
  wait_for_turns(root, &mut events, first, 1).await;
  stopping.stop_all(InterruptCause::QuitNow).await;
  fs::remove_dir_all(worktree(root, first)).expect("delete the worktree by hand");

  let (scheduler, mut events) = scheduler(root);
  let again = scheduler
    .run_again(IdempotencyKey::generate(), project.task, first, None)
    .await
    .expect("a run without its worktree no longer holds the task");
  let db = ProjectDb::open(root).expect("open");
  assert!(
    db.worktree_removal(first).expect("read").is_some(),
    "the removal is recorded once found"
  );
  assert_eq!(
    run(root, first).status(),
    &RunStatus::Interrupted(InterruptCause::QuitNow)
  );
  wait_for_turns(root, &mut events, again, 1).await;
  scheduler.stop_all(InterruptCause::QuitNow).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn run_again_of_a_task_changed_since_it_was_checked_starts_nothing() {
  let project = project(&Setup::new("Report failure."));
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler(root);
  let first = start(&scheduler, project.task).await;
  wait_for(root, &mut events, first, |run| run.status().is_terminal()).await;
  let mut db = ProjectDb::open(root).expect("open");
  let checked = db.task(project.task).expect("read").expect("task").revision();
  // The task is edited after it was checked for its next run.
  let mut task = db.task(project.task).expect("read").expect("task");
  let mut details = task.details().clone();
  details.brief = "Delete the repository.".into();
  task.edit(details, true).expect("edit");
  db.save_task(&task).expect("save").saved().expect("saved");
  assert_eq!(
    scheduler
      .run_again_as(IdempotencyKey::generate(), (project.task, first), None, Some(checked))
      .await,
    Err(Rejection::TaskChangedMeanwhile)
  );
  assert_eq!(db.runs_of_task(project.task).expect("runs").len(), 1, "nothing started");
  let now = db.task(project.task).expect("read").expect("task").revision();
  assert!(
    scheduler
      .run_again_as(IdempotencyKey::generate(), (project.task, first), None, Some(now))
      .await
      .is_ok(),
    "the task as it is now runs"
  );
}
