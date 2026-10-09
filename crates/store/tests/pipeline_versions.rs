//! Which runs pinned which published version of a pipeline, for the pipeline's version history.

mod support;

use std::{collections::BTreeMap, time::Duration};

use orchester_core::{
  id::IdempotencyKey,
  resource::{AgentProfile, Draft, Pipeline, ResourceKind, VersionNumber},
  revision::Revision,
  run::{BranchName, NewRun, PublishedVersions, RunNumber, RunSnapshot},
  store::{ResourceStore, RunStore},
};

use self::support::{at, budget, started_run};

#[test]
fn pipeline_versions_list_the_runs_that_pinned_each_version() {
  let (_repo, mut db, first, _) = started_run();
  let delivery = db.resources(ResourceKind::Pipeline).expect("pipelines")[0].id();
  let v1 = db
    .latest_version::<Pipeline>(delivery)
    .expect("read")
    .expect("v1 is published");
  let mut steps = v1.body().steps().to_vec();
  steps[0].approval_gate = true;
  let gated = Pipeline::new(steps).expect("valid pipeline");
  db.save_draft(delivery, &Draft::new(gated, Some(v1.number()), at(8)))
    .expect("save draft");
  let v2 = db.publish_draft::<Pipeline>(delivery, at(9)).expect("publish v2");
  let builder = db.resources(ResourceKind::AgentProfile).expect("profiles")[0].id();
  let profile = db
    .latest_version::<AgentProfile>(builder)
    .expect("read")
    .expect("the profile is published");
  let (profiles, tools, instructions) = (BTreeMap::from([(builder, profile)]), BTreeMap::new(), BTreeMap::new());
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &tools,
    instructions: &instructions,
    skills: &BTreeMap::new(),
  };
  let new = NewRun {
    task: first.task(),
    snapshot: RunSnapshot::build(delivery, &v2, published).expect("snapshot of v2"),
    branch: BranchName::new("orchester/orc-1-run-2").expect("branch"),
    base: Revision::new(&"b".repeat(40)).expect("revision"),
    base_branch: None,
    budget: Some(budget(1_000_000)),
    max_duration: Some(Duration::from_secs(1_800)),
  };
  let second = db
    .create_run(IdempotencyKey::generate(), new, at(10))
    .expect("run of v2");

  let runs = db.runs_by_pipeline_version(delivery).expect("runs by version");
  let expected: BTreeMap<VersionNumber, Vec<RunNumber>> = BTreeMap::from([
    (v1.number(), vec![first.number()]),
    (v2.number(), vec![second.number()]),
  ]);
  assert_eq!(runs, expected);
  // Another pipeline's versions were pinned by no run.
  assert_eq!(
    db.runs_by_pipeline_version(orchester_core::id::ResourceId::generate())
      .expect("runs of an unknown pipeline"),
    BTreeMap::new()
  );
}
