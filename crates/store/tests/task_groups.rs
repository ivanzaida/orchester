mod support;
use std::num::NonZeroU32;

use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  resource::{Pipeline, ResourceKind},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskStatus},
  task_group::{Scheduling, TaskGroup},
  task_selection::SelectionProblem,
};
use orchester_store::ProjectDb;
use support::{at, budget, name, started_run};

fn drafts(db: &mut ProjectDb) -> (ResourceId, Vec<orchester_core::task::Task>) {
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let limits = db
    .task(db.running_runs().expect("runs")[0].as_ref().expect("readable").task())
    .expect("task")
    .expect("exists")
    .details()
    .limits;
  let details = |title| TaskDetails {
    title: name(title),
    brief: "Implement this.".into(),
    acceptance_criteria: vec!["It works.".into()],
    pipeline: Some(pipeline),
    limits,
  };
  let drafts = db
    .create_tasks(IdempotencyKey::generate(), vec![details("One"), details("Two")], at(9))
    .expect("drafts");
  (pipeline, drafts)
}

#[test]
fn task_group_queue_is_atomic_pins_versions_and_is_idempotent_after_reopening() {
  let (repo, mut db, ..) = started_run();
  let (pipeline, drafts) = drafts(&mut db);
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    drafts.iter().rev().map(|t| t.id()).collect(),
    Scheduling::Parallel(NonZeroU32::new(2).expect("positive")),
    budget(2_000_000),
  )
  .expect("group");
  db.queue_task_group(&group, None).expect("queued");
  let first_version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("version")
    .expect("published")
    .number();
  drop(db);
  let mut db = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(db.queue_task_group(&group, None).expect("retry"), group);
  for task in drafts {
    let task = db.task(task.id()).expect("task").expect("exists");
    assert_eq!(task.status(), TaskStatus::Queued);
    assert_eq!(task.pipeline_version(), Some(first_version));
  }
  assert_eq!(db.task_groups().expect("groups").len(), 1);
}

#[test]
fn task_group_incomplete_selection_rolls_back_and_pipeline_assignment_skips_runs_and_queue() {
  let (_, mut db, run, _) = started_run();
  let (pipeline, drafts) = drafts(&mut db);
  let mut incomplete = drafts[1].details().clone();
  incomplete.brief.clear();
  db.update_task(drafts[1].id(), incomplete).expect("edit");
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    drafts.iter().map(|t| t.id()).collect(),
    Scheduling::Sequential,
    budget(2_000_000),
  )
  .expect("group");
  assert!(db.queue_task_group(&group, None).is_err());
  assert_eq!(
    db.task(drafts[0].id()).expect("task").expect("exists").status(),
    TaskStatus::Draft
  );
  assert!(db.task_groups().expect("groups").is_empty());
  let mut unassigned = drafts[0].details().clone();
  unassigned.pipeline = None;
  db.update_task(drafts[0].id(), unassigned).expect("clear pipeline");
  let skipped = db
    .set_tasks_pipeline(&[run.task(), drafts[0].id()], pipeline)
    .expect("assign");
  assert_eq!(skipped, vec![(run.task(), SelectionProblem::HasRun)]);
  assert_eq!(
    db.task(drafts[0].id())
      .expect("read")
      .expect("draft")
      .details()
      .pipeline,
    Some(pipeline)
  );
  let mut queued = drafts[0].clone();
  queued
    .queue(
      db.latest_version::<Pipeline>(pipeline)
        .expect("version")
        .expect("published")
        .number(),
    )
    .expect("queue");
  db.save_task(&queued).expect("save");
  assert_eq!(
    db.set_tasks_pipeline(&[queued.id()], pipeline).expect("assign"),
    vec![(queued.id(), SelectionProblem::NotDraft)]
  );
  assert!(
    db.set_tasks_pipeline(&[drafts[1].id()], ResourceId::generate())
      .is_err()
  );
}
