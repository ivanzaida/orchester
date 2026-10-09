//! The `live` scenario: the Codex connection runs the fake by an absolute path, and what it seeds can run.

use std::{collections::BTreeMap, path::PathBuf};

use orchester_core::{
  connection::{ConnectionKind, ConnectionStatus},
  id::ResourceId,
  resource::{AgentProfile, Pipeline, ResourceBody, Version},
  run::{PreflightInput, preflight},
  store::{ConnectionStore, ResourceStore, TaskStore},
};
use orchester_store::ProjectDb;

use crate::Seed;

/// `live`, its fake Codex given as a relative path. The seed only stores the program: any file stands in.
fn live() -> (Seed, PathBuf) {
  let relative = PathBuf::from("Cargo.toml");
  (
    Seed::with("live", Some(relative.clone())).expect("live seeds"),
    relative,
  )
}

/// The latest published version of every resource of kind `B`, by name.
fn published<B: ResourceBody>(db: &ProjectDb) -> BTreeMap<String, (ResourceId, Version<B>)> {
  db.resources(B::KIND)
    .expect("resources")
    .iter()
    .map(|resource| {
      let version = db.latest_version::<B>(resource.id()).expect("read").expect("published");
      (resource.name().as_str().to_owned(), (resource.id(), version))
    })
    .collect()
}

#[test]
fn seed_live_runs_the_fake_codex_by_its_absolute_path() {
  let (seed, relative) = live();
  let db = seed.db();
  let [connection] = db
    .connections()
    .expect("connections")
    .try_into()
    .expect("one connection");
  assert!(matches!(connection.kind(), ConnectionKind::Agent(_)));
  let program = connection.executable().expect("a program");
  assert!(program.is_absolute());
  assert_eq!(program, std::path::absolute(&relative).expect("absolute"));
  assert_eq!(connection.status(), ConnectionStatus::Connected);
  let profiles = published::<AgentProfile>(&db);
  assert_eq!(
    profiles.keys().collect::<Vec<_>>(),
    ["Builder", "Explorer", "Planner", "Reviewer"]
  );
  assert!(
    profiles
      .values()
      .all(|(_, profile)| profile.body().connection() == connection.id())
  );
}

#[test]
fn seed_live_pipelines_pass_the_run_preflight_and_its_drafts_are_ready() {
  let (seed, _) = live();
  let db = seed.db();
  let pipelines = published::<Pipeline>(&db);
  let gated: Vec<&str> = pipelines["Feature delivery"]
    .1
    .body()
    .steps()
    .iter()
    .filter(|step| step.approval_gate)
    .map(|step| step.key.as_str())
    .collect();
  assert_eq!(gated, ["plan"]);
  let profiles = published::<AgentProfile>(&db).into_values().collect();
  let connections = db.connections().expect("connections");
  for (name, (_, version)) in &pipelines {
    let problems = preflight(PreflightInput {
      pipeline: Some(version.body()),
      profiles: &profiles,
      tools: &BTreeMap::new(),
      connections: &connections,
    });
    assert!(problems.is_empty(), "{name} can run: {problems:?}");
  }
  let tasks = db.tasks().expect("tasks");
  assert_eq!(tasks.len(), 6);
  assert!(
    tasks.iter().all(|task| task.details().missing().is_empty()),
    "every draft is ready to run"
  );
  assert!(
    tasks
      .iter()
      .any(|task| task.details().brief.contains("Escalate `cargo add serde`"))
  );
}
