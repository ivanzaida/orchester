use std::time::Duration;

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, ResourceId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  store::{ResourceStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  tokens::TokenBudget,
};
use orchester_exec::programs::Programs;
use orchester_store::ProjectDb;
use orchester_workspace::tasks::{
  data::{ProjectTasks, SaveTarget, pinned_pipeline, published_pipelines},
  list::PipelineLabel,
  run::queue_in,
};

use super::{now, project_db};

fn research_step() -> Step {
  Step {
    key: StepKey::new("research").expect("valid key"),
    name: Name::new("Research").expect("valid name"),
    kind: StepKind::Agent {
      profile: Some(ResourceId::generate()),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn published_profile(db: &mut ProjectDb, slug: &str, role: &str, model: &str) -> ResourceId {
  let profile = AgentProfile::new(
    Name::new(role).expect("valid role"),
    String::new(),
    PromptText::new("").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new(model).expect("valid model"),
    ProviderSettings::default(),
  );
  let resource = db
    .create_resource(
      Slug::new(slug).expect("valid slug"),
      Name::new(role).expect("valid name"),
      &profile,
      now(),
    )
    .expect("create profile");
  db.publish_draft::<AgentProfile>(resource.id(), now())
    .expect("publish profile");
  resource.id()
}

#[test]
fn only_published_pipelines_can_be_chosen() {
  let (repo, mut db) = project_db();
  let pipeline = Pipeline::new(vec![research_step()]).expect("valid pipeline");
  let create = |db: &mut ProjectDb, slug: &str, name: &str| {
    db.create_resource(
      Slug::new(slug).expect("valid slug"),
      Name::new(name).expect("valid name"),
      &pipeline,
      now(),
    )
    .expect("create pipeline")
  };
  let published = create(&mut db, "delivery", "Delivery");
  create(&mut db, "draft-only", "Draft only");
  db.publish_draft::<Pipeline>(published.id(), now()).expect("publish");

  let choices = published_pipelines(&db).expect("read pipelines");
  assert_eq!(choices.len(), 1);
  assert_eq!(&*choices[0].name, "Delivery");
  assert_eq!(choices[0].version.get(), 1);
  assert_eq!(
    choices[0].steps.iter().map(|step| &*step.name).collect::<Vec<_>>(),
    ["Research"]
  );
  assert_eq!(
    PipelineLabel::of(Some(published.id()), &choices).message().key,
    "tasks.pipeline.published"
  );
  assert_eq!(
    PipelineLabel::of(Some(ResourceId::generate()), &choices),
    PipelineLabel::Unavailable
  );

  let loaded = ProjectTasks::load(repo.path(), &Programs::processes()).expect("load the project's tasks");
  assert_eq!(loaded.pipelines.len(), 1);
  assert!(loaded.tasks.is_empty());
}

#[test]
fn queued_pipeline_choice_keeps_its_published_steps_after_republish() {
  let (_repo, mut db) = project_db();
  let pipeline = Pipeline::new(vec![research_step()]).expect("valid pipeline");
  let id = db
    .create_resource(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &pipeline,
      now(),
    )
    .expect("create pipeline")
    .id();
  let v1 = db.publish_draft::<Pipeline>(id, now()).expect("publish v1");
  let mut changed = pipeline.steps().to_vec();
  changed[0].name = Name::new("Research updated").expect("name");
  let mut draft = v1.to_draft(now());
  draft.edit(Pipeline::new(changed).expect("valid pipeline"), now());
  db.save_draft(id, &draft).expect("save v2 draft");
  db.publish_draft::<Pipeline>(id, now()).expect("publish v2");

  let pinned = pinned_pipeline(&db, id, v1.number())
    .expect("read v1")
    .expect("v1 exists");
  let latest = published_pipelines(&db).expect("latest");
  assert_eq!(pinned.version, v1.number());
  assert_eq!(&*pinned.steps[0].name, "Research");
  assert_eq!(&*latest[0].steps[0].name, "Research updated");
}

#[test]
fn replaying_a_queued_batch_returns_the_original_pin_after_republish() {
  let (repo, mut db) = project_db();
  let pipeline = Pipeline::new(vec![research_step()]).expect("pipeline");
  let id = db
    .create_resource(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &pipeline,
      now(),
    )
    .expect("create pipeline")
    .id();
  let v1 = db.publish_draft::<Pipeline>(id, now()).expect("publish v1");
  let details = TaskDetails {
    title: Name::new("Build feature").expect("title"),
    brief: "Complete the requested feature.".into(),
    acceptance_criteria: vec!["It works.".into()],
    pipeline: Some(id),
    limits: TaskLimits::new(
      Some(TokenBudget::new(1_000).expect("budget")),
      Some(Duration::from_secs(60)),
    )
    .expect("limits"),
  };
  let batch = IdempotencyKey::generate();
  let mut queued = db
    .create_tasks(batch, vec![details.clone()], now())
    .expect("create task")
    .remove(0);
  queued.queue(v1.number()).expect("queue v1");
  db.save_task(&queued)
    .expect("save task")
    .saved()
    .expect("saved as read");
  let mut changed = pipeline.steps().to_vec();
  changed[0].name = Name::new("Research updated").expect("name");
  let mut draft = v1.to_draft(now());
  draft.edit(Pipeline::new(changed).expect("pipeline"), now());
  db.save_draft(id, &draft).expect("save draft");
  let v2 = db.publish_draft::<Pipeline>(id, now()).expect("publish v2");
  drop(db);

  let (_data, servers) = servers();
  let replay =
    queue_in(repo.path(), &servers, SaveTarget::New(batch), details, v2.number()).expect("replay queued task");
  assert_eq!(replay.task.id(), queued.id());
  assert_eq!(replay.task.pipeline_version(), Some(v1.number()));
  assert_eq!(replay.pipeline.version, v1.number());
  assert_eq!(&*replay.pipeline.steps[0].name, "Research");
}

#[test]
fn published_pipeline_choices_include_executor_and_approval_gate() {
  let (_repo, mut db) = project_db();
  let explorer = published_profile(&mut db, "explorer", "Explorer", "gpt-5.5");
  let builder = published_profile(&mut db, "builder", "Builder", "gpt-5.4");
  let research = Step {
    key: StepKey::new("research").expect("valid key"),
    name: Name::new("Research").expect("valid name"),
    kind: StepKind::Agent {
      profile: Some(explorer),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: Default::default(),
    approval_gate: true,
    on_failure: None,
    budget: None,
  };
  let implement = Step {
    key: StepKey::new("implement").expect("valid key"),
    name: Name::new("Implement").expect("valid name"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: Some(ModelId::new("gpt-5.4-mini").expect("valid model")),
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: [StepKey::new("research").expect("valid key")].into(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![research, implement]).expect("valid pipeline");
  let resource = db
    .create_resource(
      Slug::new("delivery").expect("valid slug"),
      Name::new("Delivery").expect("valid name"),
      &pipeline,
      now(),
    )
    .expect("create pipeline");
  db.publish_draft::<Pipeline>(resource.id(), now()).expect("publish");

  let choices = published_pipelines(&db).expect("read pipelines");
  let steps = &choices[0].steps;
  assert_eq!(&*steps[0].name, "Research");
  assert_eq!(steps[0].executor.key, "pipelines.step.profileModel");
  assert_eq!(
    steps[0].executor.args,
    [
      ("profile", "Explorer".into()),
      ("connection", String::new()),
      ("model", "gpt-5.5".into())
    ]
  );
  assert!(steps[0].approval_gate);
  assert_eq!(&*steps[1].name, "Implement");
  assert_eq!(
    steps[1].executor.args,
    [
      ("profile", "Builder".into()),
      ("connection", String::new()),
      ("model", "gpt-5.4-mini".into())
    ]
  );
  assert!(!steps[1].approval_gate);
}

/// A keychain that holds nothing: a local project's tasks never read it.
struct NoKeychain;

impl orchester_core::secret::SecretVault for NoKeychain {
  fn store(
    &self,
    _: &orchester_core::secret::VaultKey,
    _: &orchester_core::secret::SecretValue,
  ) -> Result<(), orchester_core::secret::VaultError> {
    Err(orchester_core::secret::VaultError::Unsupported)
  }

  fn read(
    &self,
    _: &orchester_core::secret::VaultKey,
  ) -> Result<orchester_core::secret::SecretValue, orchester_core::secret::VaultError> {
    Err(orchester_core::secret::VaultError::NotFound)
  }

  fn delete(&self, _: &orchester_core::secret::VaultKey) -> Result<(), orchester_core::secret::VaultError> {
    Err(orchester_core::secret::VaultError::Unsupported)
  }
}

/// The servers of a desktop that was never signed in to one, on its application data.
fn servers() -> (tempfile::TempDir, orchester_workspace::sharing::Servers) {
  let data = tempfile::TempDir::new().expect("application data");
  let workspace = orchester_workspace::workspace::Workspace::new(
    orchester_store::AppDb::open(&data.path().join("orchester.db")).expect("database"),
  );
  let servers = orchester_workspace::sharing::Servers::new(
    workspace,
    orchester_workspace::secrets::Vault::with(std::sync::Arc::new(NoKeychain)),
  );
  (data, servers)
}
