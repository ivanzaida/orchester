use std::collections::BTreeSet;

use orchester_core::{
  name::Name,
  permission::{RepositoryAccess, StepPermissions},
  resource::{Draft, ModelId, Pipeline, PromptText, Slug, Step, StepKey, StepKind},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::harness::Agents;

#[test]
fn agents_usage_includes_published_and_draft_steps_with_model_overrides() {
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
    .create_resource(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &pipeline,
      Timestamp::now(),
    )
    .expect("pipeline resource");
  let version = db
    .publish_draft::<Pipeline>(resource.id(), Timestamp::now())
    .expect("publish pipeline");
  let mut second = step;
  second.key = StepKey::new("review").expect("key");
  second.name = Name::new("Review").expect("name");
  if let StepKind::Agent { model, .. } = &mut second.kind {
    *model = Some(ModelId::new("review-model").expect("model"));
  }
  let draft = Pipeline::new(vec![pipeline.steps()[0].clone(), second]).expect("draft");
  db.save_draft(
    resource.id(),
    &Draft::new(draft, Some(version.number()), Timestamp::now()),
  )
  .expect("save draft");
  screen.open(profile);
  let text = screen.text("agent-editor");
  for expected in [
    "Delivery",
    "Published v1",
    "Draft v2 saved",
    "review-model",
    "Step override",
    "Drafts don’t run",
    "1 pipelines · 3 steps",
  ] {
    assert!(text.contains(expected), "{expected}: {text}");
  }
}
