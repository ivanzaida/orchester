//! The tasks a pipeline's archive touches: the ones that name it, which its confirmation lists, and the ones of the
//! runs its archive stopped, which the archived pipeline's panel names. `cargo test -p orchester-workspace --test
//! pipeline_task_use`.
use std::collections::BTreeMap;

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, ProjectId, ResourceId},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
    Version,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::{ArchiveStop, Archived, ProjectDb};
use orchester_workspace::pipelines::task_use::{TaskUse, stopped_by_archive, tasks_using};

fn project() -> (tempfile::TempDir, ProjectDb) {
  let folder = tempfile::tempdir().expect("a valid fixture");
  std::fs::create_dir(folder.path().join(".git")).expect("a valid fixture");
  let db = ProjectDb::create(
    folder.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("TEST").expect("a valid fixture"),
      Name::new("Test").expect("a valid fixture"),
    ),
  )
  .expect("a valid fixture");
  (folder, db)
}

fn pipeline(db: &mut ProjectDb, name: &str) -> ResourceId {
  let body = Pipeline::new(vec![]).expect("a valid fixture");
  db.create_resource(
    Slug::from_name(name).expect("a valid fixture"),
    Name::new(name).expect("a valid fixture"),
    &body,
    Timestamp::now(),
  )
  .expect("a valid fixture")
  .id()
}

fn task(db: &mut ProjectDb, title: &str, pipeline: Option<ResourceId>) -> orchester_core::task::Task {
  let details = TaskDetails {
    title: Name::new(title).expect("a valid fixture"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline,
    limits: TaskLimits::NONE,
  };
  db.create_tasks(IdempotencyKey::generate(), vec![details], Timestamp::now())
    .expect("a valid fixture")
    .remove(0)
}

#[tokio::test]
async fn task_use_lists_the_tasks_that_name_the_pipeline_by_number() {
  let (folder, mut db) = project();
  let (docs, other) = (pipeline(&mut db, "Docs"), pipeline(&mut db, "Other"));
  task(&mut db, "Write the guide", Some(docs));
  task(&mut db, "Fix the build", Some(other));
  task(&mut db, "Tidy the readme", Some(docs));
  task(&mut db, "No pipeline yet", None);

  let using = tasks_using(folder.path(), docs).await.expect("reads");
  assert_eq!(
    using,
    vec![
      TaskUse {
        key: "TEST-1".to_owned(),
        title: "Write the guide".to_owned()
      },
      TaskUse {
        key: "TEST-3".to_owned(),
        title: "Tidy the readme".to_owned()
      },
    ]
  );
  assert!(
    tasks_using(folder.path(), ResourceId::generate())
      .await
      .expect("reads")
      .is_empty(),
    "a pipeline no task names has none"
  );
}

#[tokio::test]
async fn task_use_names_the_tasks_of_the_runs_an_archive_stopped() {
  let (folder, mut db) = project();
  let (docs, version, profiles) = published_pipeline(&mut db);
  let snapshot = RunSnapshot::build(
    docs,
    &version,
    PublishedVersions {
      profiles: &profiles,
      tools: &BTreeMap::new(),
      instructions: &BTreeMap::new(),
      skills: &BTreeMap::new(),
    },
  )
  .expect("a valid fixture");
  let used = task(&mut db, "Write the guide", Some(docs));
  let run = db
    .create_run(
      IdempotencyKey::generate(),
      NewRun {
        task: used.id(),
        snapshot,
        branch: BranchName::new("orchester/test-1-run-1").expect("a valid fixture"),
        base: Revision::new(&"a".repeat(40)).expect("a valid fixture"),
        base_branch: None,
        base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
        budget: None,
        max_duration: None,
      },
      Timestamp::now(),
    )
    .expect("a valid fixture");
  assert!(
    stopped_by_archive(folder.path(), docs).await.expect("reads").is_empty(),
    "nothing was archived, so nothing was stopped"
  );
  db.record_archive_stop(&ArchiveStop {
    run: run.id(),
    task: used.id(),
    resource: Name::new("Docs").expect("a valid fixture"),
    stopped_at: Timestamp::now(),
    archived: Some(Archived {
      id: docs,
      kind: orchester_core::resource::ResourceKind::Pipeline,
      by: None,
    }),
  })
  .expect("recorded");

  let stopped = stopped_by_archive(folder.path(), docs).await.expect("reads");
  assert_eq!(stopped.len(), 1);
  assert_eq!(
    (stopped[0].key.as_str(), stopped[0].title.as_str()),
    ("TEST-1", "Write the guide")
  );
  assert!(
    stopped_by_archive(folder.path(), ResourceId::generate())
      .await
      .expect("reads")
      .is_empty(),
    "another pipeline's archive stopped none"
  );
}

/// A published pipeline called Docs whose one step runs a published profile.
fn published_pipeline(
  db: &mut ProjectDb,
) -> (
  ResourceId,
  Version<Pipeline>,
  BTreeMap<ResourceId, Version<AgentProfile>>,
) {
  let now = Timestamp::now();
  let profile = AgentProfile::new(
    Name::new("Builder").expect("a valid fixture"),
    String::new(),
    PromptText::new("Build.").expect("a valid fixture"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("a valid fixture"),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(
      Slug::new("builder").expect("a valid fixture"),
      Name::new("Builder").expect("a valid fixture"),
      &profile,
      now,
    )
    .expect("a valid fixture")
    .id();
  let profiles = BTreeMap::from([(
    builder,
    db.publish_draft::<AgentProfile>(builder, now).expect("published"),
  )]);
  let body = Pipeline::new(vec![Step {
    key: StepKey::new("implement").expect("a valid fixture"),
    name: Name::new("Implement").expect("a valid fixture"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("a valid fixture"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }])
  .expect("a valid fixture");
  let docs = db
    .create_resource(
      Slug::new("docs").expect("a valid fixture"),
      Name::new("Docs").expect("a valid fixture"),
      &body,
      now,
    )
    .expect("a valid fixture")
    .id();
  (
    docs,
    db.publish_draft::<Pipeline>(docs, now).expect("published"),
    profiles,
  )
}
