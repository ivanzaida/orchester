//! A started run of a local task, made through the store's APIs: a run that has not ended, which closing its task must
//! stop first.

use std::collections::BTreeMap;

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, TaskId},
  integration::base::RunBaseSource,
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, RunSnapshot},
  store::{ResourceStore, RunStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

pub fn start_run(db: &mut ProjectDb, task: TaskId) {
  let now = Timestamp::now();
  let snapshot = snapshot(db);
  let mut run = db
    .create_run(
      IdempotencyKey::generate(),
      NewRun {
        task,
        snapshot,
        branch: BranchName::new("orchester/kon-1-run-1").expect("branch"),
        base: Revision::new(&"a".repeat(40)).expect("revision"),
        base_branch: None,
        base_source: RunBaseSource::CheckedOut,
        budget: None,
        max_duration: None,
      },
      now,
    )
    .expect("run");
  let events = run.start(now).expect("start");
  db.append_events(&run, &events).expect("started run");
}

fn snapshot(db: &mut ProjectDb) -> RunSnapshot {
  let now = Timestamp::now();
  let name = Name::new("Builder").expect("name");
  let profile = AgentProfile::new(
    name.clone(),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name, &profile, now)
    .expect("profile")
    .id();
  let profiles = BTreeMap::from([(
    builder,
    db.publish_draft::<AgentProfile>(builder, now).expect("publish profile"),
  )]);
  let pipeline = Pipeline::new(vec![Step {
    key: StepKey::new("implement").expect("key"),
    name: Name::new("Implement").expect("name"),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::ReadOnly,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("prompt"),
    depends_on: Default::default(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }])
  .expect("pipeline");
  let id = db
    .create_published(
      Slug::new("delivery").expect("slug"),
      Name::new("Delivery").expect("name"),
      &pipeline,
      now,
    )
    .expect("pipeline")
    .0
    .id();
  let version = db
    .latest_version::<Pipeline>(id)
    .expect("publish pipeline")
    .expect("published");
  RunSnapshot::build(
    id,
    &version,
    PublishedVersions {
      profiles: &profiles,
      tools: &BTreeMap::new(),
      instructions: &BTreeMap::new(),
      skills: &BTreeMap::new(),
    },
  )
  .expect("snapshot")
}
