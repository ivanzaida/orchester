//! Story points (ORC-49) estimated by the runner on auto mode's reviewer, end to end with `orchester-fake-claude` as
//! the agent and the reviewer: an estimate is stored as automation's with its reason and its tokens are counted as
//! their own kind of usage (and in the daily budget); a person's points are never estimated over; the same brief and
//! criteria are not asked about again, a changed brief is; a merged task keeps what it took beside its points and is
//! given to later estimates as an example. Needs `git`. Run with
//! `cargo test -p orchester-runner --features test-agent --test story_points story_points`.

mod support;

use orchester_core::{
  connection::AgentProvider,
  id::{IdempotencyKey, TaskId},
  integration::{MergeKind, RunMerge},
  name::Name,
  revision::Revision,
  run::{BranchName, RunStatus, TokenUsage},
  store::{ActivityStore, MergeStore, TaskStore},
  task::estimate::{EstimateOutcome, EstimateSource, StoryPoints},
  time::Timestamp,
  tokens::TokensUsed,
};
use orchester_runner::execution::Scheduler;
use orchester_store::ProjectDb;

use self::support::{Setup, TestProject, project, reviewer::with_default_reviewer, scheduler_with, wait_for};

/// What the fake estimator reports it spent: 120 + 30 (cache writes) + 7 counted.
const COUNTED: u64 = 157;

fn claude(brief: &'static str) -> TestProject {
  with_default_reviewer(project(&Setup {
    provider: AgentProvider::ClaudeCodeCli,
    ..Setup::new(brief)
  }))
}

fn points(value: u32) -> StoryPoints {
  StoryPoints::new(value).expect("on the scale")
}

async fn estimate(scheduler: &Scheduler, task: TaskId) -> EstimateOutcome {
  scheduler
    .estimate_task(task)
    .await
    .expect("the estimate request is answered")
}

fn usage_of(db: &ProjectDb) -> (TokenUsage, u64) {
  db.estimates_between(Timestamp::from_unix_millis(0), Timestamp::from_unix_millis(i64::MAX))
    .expect("estimate usage")
}

#[tokio::test(flavor = "multi_thread")]
async fn story_points_are_estimated_on_the_reviewer_and_counted_as_usage() {
  let project = claude("Small fix.");
  let root = project.repo.path();
  let (scheduler, _events) = scheduler_with(root, project.vault.clone());
  assert_eq!(
    estimate(&scheduler, project.task).await,
    EstimateOutcome::Estimated { points: points(3) }
  );
  let db = ProjectDb::open(root).expect("open");
  let task = db.task(project.task).expect("read").expect("the task");
  let estimate = task.estimate().expect("points");
  assert_eq!(
    (estimate.points(), estimate.source()),
    (points(3), EstimateSource::Automation)
  );
  assert!(
    estimate
      .reason()
      .is_some_and(|reason| reason.contains("fake estimator")),
    "{estimate:?}"
  );
  let (usage, requests) = usage_of(&db);
  assert_eq!((usage.counted(), requests), (COUNTED, 1), "its own kind of usage");
  assert_eq!(
    db.tokens_used_since(Timestamp::from_unix_millis(0))
      .expect("the day's usage"),
    TokensUsed::new(COUNTED),
    "the daily budget counts the estimator"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn story_points_a_person_set_are_never_estimated_over() {
  let project = claude("Small fix.");
  let root = project.repo.path();
  ProjectDb::open(root)
    .expect("open")
    .set_person_points(project.task, Some(points(13)), Timestamp::now())
    .expect("the person's points");
  let (scheduler, _events) = scheduler_with(root, project.vault.clone());
  assert_eq!(estimate(&scheduler, project.task).await, EstimateOutcome::PersonsValue);
  let db = ProjectDb::open(root).expect("open");
  let task = db.task(project.task).expect("read").expect("the task");
  let estimate = task.estimate().expect("points");
  assert_eq!(
    (estimate.points(), estimate.source()),
    (points(13), EstimateSource::Person)
  );
  assert_eq!(usage_of(&db).1, 0, "no model was asked");
}

#[tokio::test(flavor = "multi_thread")]
async fn story_points_are_estimated_again_once_the_brief_changes() {
  let project = claude("Small fix.");
  let root = project.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  // A draft, which a person edits; the project's own task waits queued for its first run.
  let mut details = db.task(project.task).expect("read").expect("task").details().clone();
  let draft = db
    .create_tasks(IdempotencyKey::generate(), vec![details.clone()], Timestamp::now())
    .expect("a draft")
    .remove(0);
  let (scheduler, _events) = scheduler_with(root, project.vault.clone());
  assert_eq!(
    estimate(&scheduler, draft.id()).await,
    EstimateOutcome::Estimated { points: points(3) }
  );
  assert_eq!(
    estimate(&scheduler, draft.id()).await,
    EstimateOutcome::UpToDate,
    "the same brief and criteria ask nothing"
  );
  details.title = Name::new("Renamed only").expect("title");
  let renamed = db
    .update_task(draft.id(), draft.revision(), details.clone())
    .expect("saved")
    .saved()
    .expect("no conflict");
  assert_eq!(
    estimate(&scheduler, draft.id()).await,
    EstimateOutcome::UpToDate,
    "a new title alone asks nothing"
  );
  details.brief = "Large rewrite of the storage layer.".to_owned();
  db.update_task(renamed.id(), renamed.revision(), details)
    .expect("saved")
    .saved()
    .expect("no conflict");
  assert_eq!(
    estimate(&scheduler, draft.id()).await,
    EstimateOutcome::Estimated { points: points(8) }
  );
  assert_eq!(usage_of(&ProjectDb::open(root).expect("open")).1, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn story_points_of_a_merged_task_calibrate_later_estimates() {
  let project = claude("Small fix.");
  let root = project.repo.path();
  let (scheduler, mut events) = scheduler_with(root, project.vault.clone());
  estimate(&scheduler, project.task).await;
  let run = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("the run starts");
  let ended = wait_for(root, &mut events, run, |run| run.status().is_terminal()).await;
  assert_eq!(*ended.status(), RunStatus::Succeeded);
  let mut db = ProjectDb::open(root).expect("open");
  db.save_run_merge(&RunMerge {
    run,
    into: BranchName::new("main").expect("branch"),
    revision: Revision::new(&"c".repeat(40)).expect("revision"),
    kind: MergeKind::FastForward,
    merged_at: Timestamp::now(),
  })
  .expect("merged");
  let records = db.calibration_records().expect("calibration");
  assert_eq!(records.len(), 1, "{records:?}");
  let merged = &records[0];
  assert_eq!(merged.points, Some((points(3), EstimateSource::Automation)));
  assert_eq!((merged.effort.runs, merged.effort.attempts), (1, 1));
  assert!(merged.effort.tokens > 0, "the agent's turns count: {merged:?}");

  let next = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![db.task(project.task).expect("read").expect("task").details().clone()],
      Timestamp::now(),
    )
    .expect("a second task")
    .remove(0);
  assert_eq!(
    estimate(&scheduler, next.id()).await,
    EstimateOutcome::Estimated { points: points(3) }
  );
  let next = ProjectDb::open(root)
    .expect("open")
    .task(next.id())
    .expect("read")
    .expect("task");
  let reason = next
    .estimate()
    .and_then(|estimate| estimate.reason())
    .unwrap_or_default();
  assert!(reason.contains("examples 1"), "the merged task is an example: {reason}");
}
