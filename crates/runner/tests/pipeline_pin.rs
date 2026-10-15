//! A queued task keeps its published pipeline version across later publications.

mod support;

use orchester_core::{
  id::IdempotencyKey,
  name::Name,
  resource::{Pipeline, VersionNumber},
  run::PreflightProblem,
  store::{ResourceStore, RunStore, TaskStore},
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
  let db = ProjectDb::open(root).expect("reopen");
  let run = db.run(run_id).expect("read run").expect("run exists");
  assert_eq!(run.snapshot().pipeline().version, v1.id());
  assert_eq!(run.snapshot().pipeline().number, v1.number());
  assert_eq!(run.snapshot().body().steps()[0].name.as_str(), "Implement");
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
  )
  .with_revision(task.revision());
  db.save_task(&legacy)
    .expect("simulate a pre-migration queued row")
    .saved()
    .expect("saved as read");
  let (scheduler, _events) = scheduler(root);
  assert_eq!(
    scheduler.start_run(IdempotencyKey::generate(), fixture.task).await,
    Err(Rejection::Preflight {
      problems: vec![PreflightProblem::PipelineVersionNotPinned],
    })
  );
  assert!(db.runs_of_task(fixture.task).expect("runs").is_empty());
}

/// A shared project's cached task starts like a local one (`docs/architecture/server.md`, "Desktop
/// data"): the runner reads the cache, runs the pipeline the slug names here at the queued pin, and
/// the task stays shared.
#[tokio::test(flavor = "multi_thread")]
async fn shared_queued_task_runs_from_the_cache_unchanged() {
  use orchester_core::{
    id::{ProjectId, ServerId},
    project::ProjectKey,
    resource::ResourceKind,
    sharing::MemberRole,
  };
  use orchester_store::{CachedTask, SharingLink};

  let fixture = project(&Setup::new("Do one task."));
  let root = fixture.repo.path();
  let mut db = ProjectDb::open(root).expect("open");
  let task = db.task(fixture.task).expect("task").expect("queued task");
  let pin = task.pipeline_version();
  let pipeline = db
    .resources(ResourceKind::Pipeline)
    .expect("pipelines")
    .into_iter()
    .find(|resource| Some(resource.id()) == task.details().pipeline)
    .expect("the task's pipeline");
  let details = task.details();
  let now = Timestamp::now();
  let cached = CachedTask {
    id: task.id(),
    number: task.key().number(),
    revision: task.revision(),
    title: details.title.clone(),
    brief: details.brief.clone(),
    criteria: details.acceptance_criteria.clone(),
    pipeline: Some(pipeline.slug().as_str().to_owned()),
    limits: details.limits,
    status: task.status(),
    organization: db.task_organization(task.id()).expect("organization"),
    former_key: None,
    created_by: "Alice Moreau".into(),
    created_at: task.created_at(),
    updated_by: "Alice Moreau".into(),
    updated_at: now,
    claim: None,
    run: None,
  };
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: now,
    cursor: 0,
    last_synced_at: now,
    removed_at: None,
    address: None,
  };
  db.start_sharing(&link, &[], (&[cached], &Default::default()))
    .expect("share");
  let shared = db.task(fixture.task).expect("task").expect("cached task");
  assert_eq!(
    shared.details(),
    task.details(),
    "the cache names the same pipeline by slug"
  );
  assert_eq!(shared.pipeline_version(), pin);
  drop(db);

  let (scheduler, _events) = scheduler(root);
  let run_id = scheduler
    .start_run(IdempotencyKey::generate(), fixture.task)
    .await
    .expect("a cached task starts");
  let db = ProjectDb::open(root).expect("reopen");
  let run = db.run(run_id).expect("read run").expect("run exists");
  assert_eq!(Some(run.snapshot().pipeline().number), pin);
  assert_eq!(db.run(run_id).expect("read").map(|run| run.task()), Some(fixture.task));
  assert!(
    db.shared_task_info(fixture.task).expect("info").is_some(),
    "the task stays shared"
  );
  scheduler.wait_for_current_tasks().await;
}
