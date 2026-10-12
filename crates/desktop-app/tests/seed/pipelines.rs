//! Supported pipeline fixture contracts for independent native verification.
use orchester_core::{
  permission::{StepCommands, StepNetwork},
  resource::{AgentProfile, Pipeline, ResourceKind, StepKind},
  store::ResourceStore,
};

use super::Seed;

#[test]
fn seed_pipelines_empty_has_no_library_entries() {
  let db = Seed::of("pipelines-empty").db();
  assert!(db.resources(ResourceKind::Pipeline).unwrap().is_empty());
}
#[test]
fn seed_pipelines_reaches_saved_draft_empty_problems_and_narrowing() {
  let seed = Seed::of("pipelines");
  let db = seed.db();
  let rows = db.resources(ResourceKind::Pipeline).unwrap();
  let id = |name: &str| rows.iter().find(|r| r.name().as_str() == name).unwrap().id();
  let feature = id("Feature delivery");
  assert_eq!(
    db.latest_version::<Pipeline>(feature).unwrap().unwrap().number().get(),
    3
  );
  assert_eq!(
    db.draft::<Pipeline>(feature).unwrap().unwrap().body().description(),
    "Important saved unpublished work"
  );
  assert!(
    db.draft::<Pipeline>(id("Empty draft"))
      .unwrap()
      .unwrap()
      .body()
      .steps()
      .is_empty()
  );
  let problems =
    orchester_workspace::pipelines::data::OpenedPipeline::load_in(seed.repo(), id("Publication problems")).unwrap();
  let edit = orchester_core::resource::PipelineEdit::new(problems.body.clone());
  let unavailable = problems
    .body
    .steps()
    .iter()
    .filter(|step| orchester_workspace::pipelines::narrowing::check_step(step, &problems.assignables).is_err())
    .count();
  assert_eq!(edit.problems().len() + unavailable, 3);
  let profiles = db.resources(ResourceKind::AgentProfile).unwrap();
  let unpublished = profiles
    .iter()
    .find(|r| r.name().as_str() == "Unpublished reviewer")
    .unwrap()
    .id();
  assert!(db.latest_version::<AgentProfile>(unpublished).unwrap().is_none());
  let narrowed = db
    .latest_version::<Pipeline>(id("Narrowed permissions"))
    .unwrap()
    .unwrap();
  let step = &narrowed.body().steps()[0];
  assert!(step.approval_gate);
  let StepKind::Agent { permissions, .. } = &step.kind else {
    panic!("agent step")
  };
  assert!(matches!(&permissions.commands, StepCommands::Only(commands) if commands.entries().len() == 1));
  assert_eq!(permissions.network, StepNetwork::AsProfile);
  assert!(permissions.interactive_tool_approvals);
  let assignables = orchester_workspace::pipelines::data::Assignables::load(&db).unwrap();
  orchester_workspace::pipelines::narrowing::check_step(step, &assignables).unwrap();
}
