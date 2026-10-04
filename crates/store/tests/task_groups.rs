mod support;
use std::num::NonZeroU32;

use orchester_core::{
  id::{IdempotencyKey, ResourceId},
  resource::{Pipeline, ResourceKind},
  run::{BranchName, NewRun},
  store::{ResourceStore, RunStore, TaskStore},
  task::{MissingDetail, TaskDetails, TaskStatus},
  task_group::{GroupHold, Scheduling, TaskGroup},
  task_selection::SelectionProblem,
  tokens::TokensUsed,
};
use orchester_store::{ProjectDb, StoreError};
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
fn task_group_cap_and_usage_belong_only_to_first_runs_after_reopening() {
  let (repo, mut db, template, _) = started_run();
  let (_, drafts) = drafts(&mut db);
  let task = drafts[0].id();
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    vec![task],
    Scheduling::Sequential,
    budget(100),
  )
  .expect("group");
  db.queue_task_group(&group, None, &reviewed(&db, &group))
    .expect("queue");
  let new = |branch| NewRun {
    task,
    snapshot: template.snapshot().clone(),
    branch: BranchName::new(branch).expect("branch"),
    base: template.base().clone(),
    base_branch: None,
    budget: template.budget(),
    max_duration: template.max_duration(),
  };
  let mut first = db
    .create_run(task.first_run_key(), new("orchester/orc-2-run-1"), at(10))
    .expect("first");
  let events = first.start(at(11)).expect("start");
  db.append_events(&first, &events).expect("events");
  let events = first.record_usage(TokensUsed::new(100), at(12)).expect("usage");
  db.append_events(&first, &events).expect("events");
  assert!(db.task_group_limit_reached(&first).expect("first cap"));
  let events = first.cancel(at(13)).expect("cancel");
  db.append_events(&first, &events).expect("events");
  let mut again = db
    .create_run(IdempotencyKey::generate(), new("orchester/orc-2-run-2"), at(14))
    .expect("again");
  assert!(!db.task_group_limit_reached(&again).expect("new pending run cap"));
  let events = again.start(at(15)).expect("start");
  db.append_events(&again, &events).expect("events");
  let events = again.record_usage(TokensUsed::new(300), at(16)).expect("usage");
  db.append_events(&again, &events).expect("events");
  drop(db);
  let mut db = ProjectDb::open(repo.path()).expect("reopen");
  assert!(!db.task_group_limit_reached(&again).expect("running again cap"));
  assert!(db.task_group_for_run(&again).expect("membership").is_none());
  assert!(db.task_group_limit_reached(&first).expect("first cap persists"));
  assert_eq!(
    db.task_group_usage(&group).expect("only first usage"),
    TokensUsed::new(100)
  );
  assert_eq!(
    db.queue_task_group(&group, None, &reviewed(&db, &group))
      .expect("first-group retry"),
    group
  );
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
  db.queue_task_group(&group, None, &reviewed(&db, &group))
    .expect("queued");
  let first_version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("version")
    .expect("published")
    .number();
  drop(db);
  let mut db = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(
    db.queue_task_group(&group, None, &reviewed(&db, &group))
      .expect("retry"),
    group
  );
  for task in drafts {
    let task = db.task(task.id()).expect("task").expect("exists");
    assert_eq!(task.status(), TaskStatus::Queued);
    assert_eq!(task.pipeline_version(), Some(first_version));
  }
  assert_eq!(db.task_groups().expect("groups").len(), 1);
}

/// Saves `details` over `task` as it was read.
fn edit(db: &mut ProjectDb, task: &orchester_core::task::Task, details: orchester_core::task::TaskDetails) {
  db.update_task(task.id(), task.revision(), details)
    .expect("edit")
    .saved()
    .expect("edited as read");
}

#[test]
fn task_group_incomplete_selection_rolls_back_and_pipeline_assignment_skips_runs_and_queue() {
  let (_, mut db, run, _) = started_run();
  let (pipeline, drafts) = drafts(&mut db);
  let mut incomplete = drafts[1].details().clone();
  incomplete.brief.clear();
  edit(&mut db, &drafts[1], incomplete);
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    drafts.iter().map(|t| t.id()).collect(),
    Scheduling::Sequential,
    budget(2_000_000),
  )
  .expect("group");
  assert!(matches!(
    db.queue_task_group(&group, None, &reviewed(&db, &group)),
    Err(StoreError::TaskNotSelectable {
      problem: SelectionProblem::Missing(MissingDetail::Brief),
      ..
    })
  ));
  assert_eq!(
    db.task(drafts[0].id()).expect("task").expect("exists").status(),
    TaskStatus::Draft
  );
  assert!(db.task_groups().expect("groups").is_empty());
  let mut unassigned = drafts[0].details().clone();
  unassigned.pipeline = None;
  edit(&mut db, &drafts[0], unassigned);
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
  let mut queued = db.task(drafts[0].id()).expect("read").expect("draft");
  queued
    .queue(
      db.latest_version::<Pipeline>(pipeline)
        .expect("version")
        .expect("published")
        .number(),
    )
    .expect("queue");
  db.save_task(&queued).expect("save").saved().expect("saved as read");
  assert_eq!(
    db.set_tasks_pipeline(&[queued.id()], pipeline).expect("assign"),
    vec![(queued.id(), SelectionProblem::NotDraft)]
  );
  assert!(matches!(
    db.set_tasks_pipeline(&[drafts[1].id()], ResourceId::generate()),
    Err(StoreError::ResourceNotFound(_))
  ));
  let body = db
    .latest_version::<Pipeline>(pipeline)
    .expect("version")
    .expect("published")
    .body()
    .clone();
  let unpublished = db
    .create_resource(
      orchester_core::resource::Slug::new("unpublished").expect("slug"),
      name("Unpublished"),
      &body,
      at(10),
    )
    .expect("draft pipeline")
    .id();
  assert!(matches!(
    db.set_tasks_pipeline(&[drafts[1].id()], unpublished),
    Err(StoreError::PipelineNotPublished(_))
  ));
}

fn reviewed(db: &ProjectDb, group: &TaskGroup) -> Vec<orchester_core::task_group::ReviewedPipeline> {
  use orchester_core::{resource::Pipeline, store::ResourceStore, task_group::ReviewedPipeline};
  group
    .tasks()
    .iter()
    .map(|id| {
      let task = db.task(*id).expect("task").expect("exists");
      let pipeline = task.details().pipeline.expect("pipeline");
      let version = db
        .latest_version::<Pipeline>(pipeline)
        .expect("version")
        .expect("published")
        .number();
      ReviewedPipeline {
        task: *id,
        pipeline,
        version,
      }
    })
    .collect()
}

#[test]
fn task_group_submission_keeps_reviewed_version_after_new_publication() {
  let (_, mut db, ..) = started_run();
  let (pipeline, drafts) = drafts(&mut db);
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    drafts.iter().map(|t| t.id()).collect(),
    Scheduling::Sequential,
    budget(2_000_000),
  )
  .expect("group");
  let reviewed = reviewed(&db, &group);
  let version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("version")
    .expect("published");
  let mut steps = version.body().steps().to_vec();
  steps[0].name = name("Unseen implementation");
  let mut draft = version.to_draft(at(20));
  draft.edit(Pipeline::new(steps).expect("pipeline"), at(21));
  db.save_draft(pipeline, &draft).expect("draft");
  let newer = db.publish_draft::<Pipeline>(pipeline, at(22)).expect("publish");
  assert_ne!(newer.number(), reviewed[0].version);
  db.queue_task_group(&group, None, &reviewed)
    .expect("queue reviewed publication");
  for task in drafts {
    assert_eq!(
      db.task(task.id()).expect("task").expect("exists").pipeline_version(),
      Some(version.number())
    );
  }
  assert_eq!(db.queue_task_group(&group, None, &[]).expect("lost-reply retry"), group);
}

#[test]
fn task_group_changed_assignment_rejects_the_whole_review_atomically() {
  let (_, mut db, ..) = started_run();
  let (pipeline, drafts) = drafts(&mut db);
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    drafts.iter().map(|t| t.id()).collect(),
    Scheduling::Sequential,
    budget(2_000_000),
  )
  .expect("group");
  let reviewed = reviewed(&db, &group);
  let version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("version")
    .expect("published");
  let other = db
    .create_resource(
      orchester_core::resource::Slug::new("other").expect("slug"),
      name("Other"),
      version.body(),
      at(20),
    )
    .expect("resource");
  db.publish_draft::<Pipeline>(other.id(), at(21)).expect("publish");
  db.set_tasks_pipeline(&[drafts[1].id()], other.id()).expect("assign");
  assert!(
    matches!(db.queue_task_group(&group, None, &reviewed), Err(orchester_store::StoreError::TaskGroupReviewChanged(id)) if id == drafts[1].id())
  );
  assert!(db.task_groups().expect("groups").is_empty());
  assert!(
    db.tasks()
      .expect("tasks")
      .iter()
      .filter(|t| group.tasks().contains(&t.id()))
      .all(|t| t.status() == TaskStatus::Draft)
  );
}

#[test]
fn task_group_release_returns_held_unstarted_tasks_to_drafts_and_finishes() {
  let (_, mut db, template, _) = started_run();
  let (_, drafts) = drafts(&mut db);
  let ids: Vec<_> = drafts.iter().map(|t| t.id()).collect();
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    ids.clone(),
    Scheduling::Sequential,
    budget(2_000_000),
  )
  .expect("group");
  db.queue_task_group(&group, None, &reviewed(&db, &group))
    .expect("queue");
  // A group that still schedules its work keeps its tasks.
  db.set_task_group_hold(group.key(), Some(&GroupHold::PreviousTask))
    .expect("hold");
  assert!(matches!(
    db.release_held_tasks(&ids[1..]),
    Err(StoreError::TaskNotHeld(_))
  ));
  // Tasks waiting for blocking tasks may wait for good: they can be taken back too.
  db.set_task_group_hold(
    group.key(),
    Some(&GroupHold::Blocked {
      by: vec![orchester_core::id::TaskId::generate()],
    }),
  )
  .expect("hold");
  db.release_held_tasks(&ids[1..]).expect("release");
  let released = db.task(ids[1]).expect("task").expect("exists");
  assert_eq!(released.status(), TaskStatus::Draft);
  assert_eq!(released.pipeline_version(), None);
  assert!(db.task_group_for_task(ids[1]).expect("group").is_none());
  // A start the runner checked before the release records no run for the draft it became.
  let late = NewRun {
    task: ids[1],
    snapshot: template.snapshot().clone(),
    branch: BranchName::new("orchester/late-start").expect("branch"),
    base: template.base().clone(),
    base_branch: None,
    budget: template.budget(),
    max_duration: template.max_duration(),
  };
  assert!(matches!(
    db.create_run_of_queued_task(ids[1].first_run_key(), late, at(20)),
    Err(orchester_store::RunStoreError::TaskNotQueued(task)) if task == ids[1]
  ));
  assert!(db.run_with_key(ids[1].first_run_key()).expect("read").is_none());
  assert_eq!(
    db.task_group(group.key()).expect("group").expect("kept").tasks(),
    &ids[..1]
  );
  assert_eq!(db.unfinished_task_groups().expect("groups").len(), 1);
  db.finish_task_group(group.key()).expect("finish");
  assert!(db.unfinished_task_groups().expect("groups").is_empty());
  assert_eq!(db.task_groups().expect("groups").len(), 1);
  // Releasing the last member removes the group.
  db.release_held_tasks(&ids[..1]).expect("release the rest");
  assert!(db.task_groups().expect("groups").is_empty());
}

/// A draft whose blocking task has not finished cannot enter a group: the runner would refuse it.
#[test]
fn task_group_refuses_a_blocked_draft_and_queues_nothing() {
  use orchester_core::task::organization::{
    BlockingTask, ChangeActor, DependencyGate, Priority, TaskOrganization, WorkflowStatus,
  };
  let (_, mut db, run, _) = started_run();
  let (_, drafts) = drafts(&mut db);
  let blocked = TaskOrganization::new(
    WorkflowStatus::Todo,
    Priority::None,
    Some("Waits for the running task.".into()),
    vec![BlockingTask {
      task: run.task(),
      gate: DependencyGate::Review,
    }],
    vec![],
  )
  .expect("organization");
  db.set_task_organization(drafts[1].id(), &blocked, ChangeActor::Person, at(10))
    .expect("link");
  let group = TaskGroup::new(
    IdempotencyKey::generate(),
    drafts.iter().map(|t| t.id()).collect(),
    Scheduling::Sequential,
    budget(2_000_000),
  )
  .expect("group");
  assert!(matches!(
    db.queue_task_group(&group, None, &reviewed(&db, &group)),
    Err(StoreError::TaskNotSelectable {
      problem: SelectionProblem::Blocked { by },
      ..
    }) if by == vec![run.task()]
  ));
  assert!(db.task_groups().expect("groups").is_empty());
  assert_eq!(
    db.task(drafts[0].id()).expect("task").expect("exists").status(),
    TaskStatus::Draft
  );
}
