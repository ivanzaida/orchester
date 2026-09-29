//! A queued task keeps its published pipeline version across later publications.

mod support;

use std::{collections::BTreeMap, fs};

use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  resource::{CommandEnvironment, Pipeline, VariableName, VersionNumber},
  run::PreflightProblem,
  store::{ResourceStore, RunStore, SettingsStore, TaskStore},
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::ProjectDb;

use self::support::{Setup, project, scheduler};

#[tokio::test(flavor = "multi_thread")]
async fn queued_task_runs_its_pinned_pipeline_after_a_new_version_is_published() {
  let fixture = project(&Setup::new("Do one task."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  let task = db.task(fixture.task).expect("task").expect("queued task");
  let pipeline_id = task.details().pipeline.expect("pipeline");
  let v1 = db
    .version::<Pipeline>(pipeline_id, VersionNumber::FIRST)
    .expect("v1 lookup")
    .expect("v1");
  let mut changed = v1.body().steps().to_vec();
  changed[0].name = Name::new("Implement updated").expect("step name");
  let now = Timestamp::now();
  let mut draft = v1.to_draft(now);
  draft.edit(Pipeline::new(changed).expect("pipeline"), now);
  db.save_draft(pipeline_id, &draft).expect("save draft");
  let v2 = db.publish_draft::<Pipeline>(pipeline_id, now).expect("publish v2");
  assert_eq!(v2.number().get(), 2);
  drop(db);

  let (scheduler, _events) = scheduler(root);
  let key = IdempotencyKey::generate();
  let run_id = scheduler.start_run(key, fixture.task).await.expect("start pinned run");
  let mut db = ProjectDb::open(root).expect("reopen");
  let run = db.run(run_id).expect("read run").expect("run exists");
  assert_eq!(run.snapshot().pipeline().version, v1.id());
  assert_eq!(run.snapshot().pipeline().number, v1.number());
  assert_eq!(run.snapshot().body().steps()[0].name.as_str(), "Implement");
  let blocked = root.join("blocked-target");
  fs::write(&blocked, "not a directory").expect("block target");
  db.save_project_environment(
    &CommandEnvironment::new(BTreeMap::from([(
      VariableName::new("CARGO_TARGET_DIR").expect("variable name"),
      blocked.to_str().expect("Unicode path").into(),
    )]))
    .expect("environment"),
  )
  .expect("save blocked target");
  assert_eq!(scheduler.start_run(key, fixture.task).await, Ok(run_id));
  scheduler.wait_for_current_tasks().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn legacy_queued_task_without_a_pin_is_rejected_before_creating_a_run() {
  let fixture = project(&Setup::new("Do one task."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  let task = db.task(fixture.task).expect("task").expect("queued task");
  let legacy = orchester_core::task::Task::from_stored(
    task.id(),
    task.key().clone(),
    task.details().clone(),
    task.status(),
    None,
    task.created_at(),
  );
  db.save_task(&legacy).expect("simulate a pre-migration queued row");
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), fixture.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::PipelineVersionNotPinned],
    })
  );
  assert!(db.runs_of_task(fixture.task).expect("runs").is_empty());
}
