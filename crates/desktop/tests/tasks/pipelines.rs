use orchester_core::{
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  store::ResourceStore,
};
use orchester_desktop::tasks::{
  data::{ProjectTasks, published_pipelines},
  list::PipelineLabel,
};
use orchester_store::ProjectDb;

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

  let loaded = ProjectTasks::load(repo.path()).expect("load the project's tasks");
  assert_eq!(loaded.pipelines.len(), 1);
  assert!(loaded.tasks.is_empty());
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
