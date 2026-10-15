//! A run confirmed on a task's revision is recorded only while the task is still at it, read in the
//! transaction that records the run: an edit between the runner's check and the record starts nothing.

mod support;

use orchester_core::{
  id::IdempotencyKey,
  resource::{Pipeline, ResourceKind},
  run::{BranchName, NewRun},
  store::{ResourceStore, RunStore, TaskStore},
  task::TaskDetails,
};
use orchester_store::RunStoreError;
use support::{at, name, started_run};

#[test]
fn queued_run_of_a_task_edited_since_its_revision_was_confirmed_is_not_recorded() {
  let (_repo, mut db, template, _) = started_run();
  let pipeline = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let version = db
    .latest_version::<Pipeline>(pipeline)
    .expect("read")
    .expect("published")
    .number();
  let details = TaskDetails {
    title: name("Add login"),
    brief: "Implement this.".into(),
    acceptance_criteria: vec!["It works.".into()],
    pipeline: Some(pipeline),
    limits: db
      .task(template.task())
      .expect("task")
      .expect("exists")
      .details()
      .limits,
  };
  let mut task = db
    .create_tasks(IdempotencyKey::generate(), vec![details], at(10))
    .expect("draft")
    .remove(0);
  task.queue(version).expect("queue");
  let task = db.save_task(&task).expect("save").saved().expect("saved as read");
  let next = |branch: &str| NewRun {
    task: task.id(),
    snapshot: template.snapshot().clone(),
    branch: BranchName::new(branch).expect("branch"),
    base: template.base().clone(),
    base_branch: None,
    budget: template.budget(),
    max_duration: template.max_duration(),
  };
  db.create_run(IdempotencyKey::generate(), next("orchester/first"), at(20))
    .expect("its first run");
  // The person confirms Run again on this revision; the task is edited before the run is recorded.
  let confirmed = task.revision();
  let mut edited = db.task(task.id()).expect("task").expect("exists");
  let mut changed = edited.details().clone();
  changed.brief = "Delete the repository.".into();
  edited.edit(changed, true).expect("edit");
  db.save_task(&edited).expect("save").saved().expect("saved as read");

  let refused = db.create_run_of_queued_task(
    IdempotencyKey::generate(),
    next("orchester/again"),
    at(30),
    Some(confirmed),
  );
  assert!(
    matches!(refused, Err(RunStoreError::TaskChangedMeanwhile(id)) if id == task.id()),
    "{refused:?}"
  );
  assert_eq!(db.runs_of_task(task.id()).expect("runs").len(), 1, "nothing recorded");
  let now = db.task(task.id()).expect("task").expect("exists").revision();
  db.create_run_of_queued_task(IdempotencyKey::generate(), next("orchester/again"), at(31), Some(now))
    .expect("the task as it is now runs");
}
