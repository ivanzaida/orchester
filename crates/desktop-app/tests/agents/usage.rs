use std::collections::BTreeSet;

use orchester_core::{
  name::Name,
  permission::{RepositoryAccess, StepPermissions},
  resource::{ModelId, Pipeline, PromptText, Slug, Step, StepKey, StepKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::harness::Agents;

#[test]
fn agents_usage_reads_each_pipelines_latest_version_with_model_overrides() {
  let mut screen = Agents::new(true);
  let profile = screen.profile(true);
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: Name::new("Implement").expect("name"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Implement.").expect("instructions"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let pipeline = Pipeline::new(vec![step.clone()]).expect("pipeline");
  let mut db = ProjectDb::open(&screen.root).expect("database");
  let resource = db
    .create_published(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &pipeline,
      Timestamp::now(),
    )
    .expect("pipeline resource")
    .0;
  let version = db
    .latest_version::<Pipeline>(resource.id())
    .expect("publish pipeline")
    .expect("published");
  let mut second = step;
  second.key = StepKey::new("review").expect("key");
  second.name = Name::new("Review").expect("name");
  if let StepKind::Agent { model, .. } = &mut second.kind {
    *model = Some(ModelId::new("review-model").expect("model"));
  }
  // A pipeline holds no draft (D-032): its uses are its latest version's, v2.
  let next = Pipeline::new(vec![pipeline.steps()[0].clone(), second]).expect("v2");
  db.publish_pipeline_edit(resource.id(), &next, version.number(), None, Timestamp::now())
    .expect("v2");
  screen.open(profile);
  let text = screen.text("agent-editor");
  for expected in [
    "Delivery",
    "Published v1",
    "Delivery v2",
    "review-model",
    "Step override",
    "1 pipelines · 2 steps",
  ] {
    assert!(text.contains(expected), "{expected}: {text}");
  }
}
