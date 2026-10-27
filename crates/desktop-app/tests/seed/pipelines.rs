//! Supported pipeline fixture contracts for independent native verification.
use orchester_core::{
  permission::{StepCommands, StepNetwork},
  resource::{AgentProfile, Pipeline, ResourceKind, StepKind, ToolKind},
  store::ResourceStore,
};
use orchester_store::StoreError;
use orchester_workspace::pipelines::{
  data::{Assignables, OpenedPipeline},
  deliver::integrator_profiles,
  narrowing,
};

use super::Seed;

#[test]
fn seed_pipelines_holds_the_step_variants_no_other_data_reaches() {
  let seed = Seed::of("pipelines");
  let db = seed.db();
  let rows = db.resources(ResourceKind::Pipeline).unwrap();
  let opened = |name: &str| {
    let id = rows.iter().find(|r| r.name().as_str() == name).unwrap().id();
    OpenedPipeline::load_in(seed.repo(), id).unwrap()
  };
  let status = opened("Status checks");
  let [step] = status.body.steps() else {
    panic!("one step")
  };
  let StepKind::Tool { tool: Some(tool) } = step.kind else {
    panic!("a tool step")
  };
  let tool = status.assignables.tool(tool).expect("a published tool");
  assert_eq!((tool.kind, tool.runnable), (ToolKind::Http, false));
  let review = opened("Read-only review");
  let [first, second] = review.body.steps() else {
    panic!("two steps")
  };
  assert_eq!(
    second.depends_on,
    [first.key.clone()].into(),
    "a cycle is one choice away"
  );
  let StepKind::Agent {
    profile: Some(profile), ..
  } = first.kind
  else {
    panic!("an agent step")
  };
  let codex = review.assignables.profile(profile).expect("a published profile");
  assert!(narrowing::commands(codex).is_empty(), "no command list to narrow");
  assert!(!narrowing::can_write(codex));
  if cfg!(windows) {
    assert!(
      !narrowing::can_block_network(codex),
      "Codex cannot turn network off on Windows"
    );
  }
  let deliver = review.body.deliver().expect("a Deliver step");
  assert!(deliver.recheck() && !deliver.recheck_locked());
  assert!(integrator_profiles(&review.body).is_empty(), "no step changes files");
  let tools = Assignables::load(&db).unwrap();
  assert!(tools.tools.iter().any(|t| t.kind == ToolKind::Database && !t.runnable));
}

#[test]
fn seed_sources_unreadable_lists_a_path_no_read_takes() {
  let db = Seed::of("sources-unreadable").db();
  assert!(
    matches!(db.project_sources(), Err(StoreError::Corrupt { .. })),
    "{:?}",
    db.project_sources()
  );
}

#[test]
fn seed_pipelines_empty_has_no_library_entries() {
  let db = Seed::of("pipelines-empty").db();
  assert!(db.resources(ResourceKind::Pipeline).unwrap().is_empty());
}

#[test]
fn seed_pipelines_publish_saves_a_draft_whose_publishing_asks() {
  let seed = Seed::of("pipelines-publish");
  let db = seed.db();
  let rows = db.resources(ResourceKind::Pipeline).unwrap();
  let id = rows
    .iter()
    .find(|r| r.name().as_str() == "Feature delivery")
    .unwrap()
    .id();
  let opened = OpenedPipeline::load_in(seed.repo(), id).unwrap();
  assert!(opened.draft_saved_at.is_some(), "a saved draft");
  let latest = opened.latest.as_ref().expect("published");
  assert_eq!(latest.number.get(), 3);
  let reasons = orchester_workspace::pipelines::widening::publish_reasons(Some(&latest.body), &opened.body);
  assert_eq!(reasons.len(), 1, "Implement runs on another model: {reasons:?}");
  assert_eq!(
    orchester_core::resource::changes(&latest.body, &opened.body).len(),
    2,
    "Implement and Deliver changed"
  );
}

#[test]
fn seed_import_done_has_an_imported_pipeline_with_a_secret_missing() {
  let db = Seed::of("import-done").db();
  let rows = db.resources(ResourceKind::Pipeline).unwrap();
  let copy = rows
    .iter()
    .find(|r| r.name().as_str() == "Preview release")
    .expect("the imported copy")
    .id();
  assert!(
    db.latest_version::<Pipeline>(copy).unwrap().is_none(),
    "not published yet"
  );
  let imported = orchester_workspace::import::imported::imported(&db, copy)
    .unwrap()
    .expect("what its import did");
  assert_eq!(imported.project, "Personal website");
  assert!(!imported.with_it.is_empty(), "something came along");
  let missing: Vec<(bool, &str)> = imported
    .missing
    .iter()
    .map(|missing| {
      (
        matches!(missing.source, orchester_core::import::Entity::Secret(_)),
        missing.name.as_str(),
      )
    })
    .collect();
  assert_eq!(missing, [(true, "PREVIEW_HOST_API_KEY")]);
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
