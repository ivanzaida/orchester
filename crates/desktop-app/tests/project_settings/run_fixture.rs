//! An existing started run in the window's own project, in Manual, created through store APIs.
use std::collections::BTreeMap;

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, RunId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey, StepKind,
  },
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, RunSnapshot},
  store::{ResourceStore, RunStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::ProjectDb;

pub fn started_run(db: &mut ProjectDb) -> RunId {
  let mut run = runs(db, 1).remove(0);
  let events = run.start(Timestamp::now()).expect("start");
  db.append_events(&run, &events).expect("started run");
  run.id()
}

/// `count` runs of one task, not started: what a history of runs refers to.
pub fn run_ids(db: &mut ProjectDb, count: usize) -> Vec<RunId> {
  runs(db, count).iter().map(|run| run.id()).collect()
}

fn runs(db: &mut ProjectDb, count: usize) -> Vec<orchester_core::run::Run> {
  let now = Timestamp::now();
  let task = db
    .create_tasks(
      IdempotencyKey::generate(),
      vec![TaskDetails {
        title: Name::new("Reviewer regression").expect("name"),
        brief: String::new(),
        acceptance_criteria: Vec::new(),
        pipeline: None,
        limits: TaskLimits::NONE,
      }],
      now,
    )
    .expect("task")[0]
    .id();
  let snapshot = snapshot(db);
  (1..=count)
    .map(|number| {
      db.create_run(
        IdempotencyKey::generate(),
        NewRun {
          task,
          snapshot: snapshot.clone(),
          branch: BranchName::new(&format!("orchester/kon-1-run-{number}")).expect("branch"),
          base: Revision::new(&"a".repeat(40)).expect("revision"),
          base_branch: None,
          base_source: orchester_core::integration::base::RunBaseSource::CheckedOut,
          budget: None,
          max_duration: None,
        },
        now,
      )
      .expect("run")
    })
    .collect()
}

fn snapshot(db: &mut ProjectDb) -> RunSnapshot {
  let now = Timestamp::now();
  let name = Name::new("Regression builder").expect("name");
  let profile = AgentProfile::new(
    name.clone(),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(Slug::new("regression-builder").expect("slug"), name, &profile, now)
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
    .create_resource(
      Slug::new("regression-delivery").expect("slug"),
      Name::new("Regression delivery").expect("name"),
      &pipeline,
      now,
    )
    .expect("pipeline")
    .id();
  let version = db.publish_draft::<Pipeline>(id, now).expect("publish pipeline");
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
