//! Supported pipeline fixture contracts for independent native verification.
use orchester_core::{
  connection::AgentProvider,
  permission::{StepCommands, StepNetwork},
  resource::{AgentProfile, Pipeline, ResourceKind, StepKind, Tool, ToolKind},
  store::ResourceStore,
};
use orchester_store::StoreError;
use orchester_workspace::pipelines::{
  data::{Assignables, OpenedPipeline},
  deliver::integrator_profiles,
  flow::{PipelineFlow, Runs},
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
fn seed_import_done_has_an_imported_tool_with_a_secret_missing() {
  let db = Seed::of("import-done").db();
  let rows = db.resources(ResourceKind::Tool).unwrap();
  let copy = rows
    .iter()
    .find(|r| r.name().as_str() == "Preview host")
    .expect("the imported copy")
    .id();
  assert!(db.latest_version::<Tool>(copy).unwrap().is_none(), "not published yet");
  assert!(
    db.resources(ResourceKind::Pipeline)
      .unwrap()
      .iter()
      .all(|r| r.name().as_str() != "Preview release"),
    "pipelines are not imported"
  );
  let imported = orchester_workspace::import::imported::imported(&db, copy)
    .unwrap()
    .expect("what its import did");
  assert_eq!(imported.project, "Personal website");
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
fn seed_pipelines_reaches_problems_and_narrowing_with_no_pipeline_draft() {
  let seed = Seed::of("pipelines");
  let db = seed.db();
  let rows = db.resources(ResourceKind::Pipeline).unwrap();
  let id = |name: &str| rows.iter().find(|r| r.name().as_str() == name).unwrap().id();
  let feature = id("Feature delivery");
  assert_eq!(
    db.latest_version::<Pipeline>(feature).unwrap().unwrap().number().get(),
    3
  );
  for row in &rows {
    assert!(
      db.draft::<Pipeline>(row.id()).unwrap().is_none(),
      "a pipeline holds no draft (D-032): {}",
      row.name().as_str()
    );
    assert!(db.latest_version::<Pipeline>(row.id()).unwrap().is_some());
  }
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

#[test]
fn seed_pipelines_flow_reaches_every_state_the_steps_are_drawn_in() {
  let seed = Seed::of("pipelines-flow");
  let db = seed.db();
  let rows = db.resources(ResourceKind::Pipeline).unwrap();
  let flow = |name: &str| {
    let id = rows.iter().find(|r| r.name().as_str() == name).unwrap().id();
    let opened = OpenedPipeline::load_in(seed.repo(), id).unwrap();
    PipelineFlow::of(&opened.body, &opened.assignables)
  };
  let feature = flow("Feature delivery").expect("Feature delivery is drawn");
  let sizes: Vec<usize> = feature.stages.iter().map(|stage| stage.cards.len()).collect();
  assert_eq!(sizes, [1, 1, 1, 2, 1], "the checks and the review run side by side");
  assert_eq!(feature.stages[1].gates.len(), 1, "the plan waits for the person");
  assert!(feature.stages[3].cards.iter().all(|card| card.on_failure.is_some()));
  assert!(feature.deliver.is_some(), "it ends in a Deliver step");
  let waiting: Vec<_> = feature
    .stages
    .iter()
    .flat_map(|stage| &stage.cards)
    // Without a fake Codex the seed records no Codex connection, so the catalogue's profiles stand on a connection
    // this computer lacks: that is the computer's to fix, not the pipeline's.
    .filter(|card| card.runs.needs_you() && !matches!(card.runs, Runs::ProfileConnectionGone { .. }))
    .map(|card| (&card.name, &card.runs))
    .collect();
  assert!(
    waiting.is_empty(),
    "every card of Feature delivery can run: {waiting:?}"
  );

  let problems = flow("Publication problems").expect("executors that cannot run are drawn");
  let runs: Vec<&Runs> = problems.stages.iter().map(|stage| &stage.cards[0].runs).collect();
  assert!(
    matches!(runs[0], Runs::ProfileNotPublished { .. }),
    "a profile that is no longer available"
  );
  assert!(matches!(runs[1], Runs::ProfileNotPublished { .. }));
  assert!(matches!(runs[2], Runs::ToolNotRunnable { name, .. } if &**name == "Unsupported database"));
  assert_eq!(runs.len(), 3);

  let gated = flow("Gated group").expect("a gated group is drawn");
  assert_eq!((gated.stages[1].cards.len(), gated.stages[1].gates.len()), (2, 2));
  let chains = flow("Independent chains").expect("chains are drawn by stage");
  assert!(
    chains.stages[1].cards.iter().all(|card| card.waits.is_some()),
    "each chain's second step waits for its own first only"
  );
}

#[test]
fn seed_pipelines_not_ready_has_a_review_step_whose_profile_is_gone_and_three_ready_pipelines() {
  let seed = Seed::of("pipelines-not-ready");
  let db = seed.db();
  let rows = db.resources(ResourceKind::Pipeline).unwrap();
  let opened = |name: &str| {
    let id = rows.iter().find(|r| r.name().as_str() == name).unwrap().id();
    OpenedPipeline::load_in(seed.repo(), id).unwrap()
  };
  assert_eq!(rows.len(), 4);
  let update = opened("Dependency update");
  assert_eq!(
    update.version.get(),
    1,
    "published, as every stored pipeline is (D-032)"
  );
  let [first, review] = update.body.steps() else {
    panic!("two steps")
  };
  assert!(matches!(first.kind, StepKind::Agent { profile: Some(_), .. }));
  assert_eq!(review.name.as_str(), "Review");
  let StepKind::Agent {
    profile: Some(gone), ..
  } = review.kind
  else {
    panic!("Review runs a profile")
  };
  assert!(update.assignables.gone(gone), "a profile that is no longer available");
  assert!(update.body.deliver().is_some(), "Deliver is the last card");
  for name in ["Feature delivery", "Quick fix", "Release notes"] {
    let ready = opened(name);
    assert!(
      ready
        .body
        .steps()
        .iter()
        .all(|step| matches!(step.kind, StepKind::Agent { profile: Some(_), .. })),
      "{name} runs a profile in every step"
    );
  }
}

#[test]
fn seed_pipelines_agents_has_every_kind_of_agent_the_picker_lists() {
  let seed = Seed::of("pipelines-agents");
  let db = seed.db();
  let rows = db.resources(ResourceKind::Pipeline).unwrap();
  let id = rows.iter().find(|r| r.name().as_str() == "Agents tour").unwrap().id();
  let opened = OpenedPipeline::load_in(seed.repo(), id).unwrap();
  let assignables = &opened.assignables;
  let profile = |name: &str| {
    assignables
      .profiles
      .iter()
      .find(|profile| &*profile.name == name)
      .unwrap_or_else(|| panic!("the seed lists {name}"))
  };
  let (builder, notes) = (profile("Builder"), profile("Release notes"));
  assert_eq!(builder.connection.as_deref(), Some("Work"));
  assert_eq!(builder.provider, Some(AgentProvider::ClaudeCodeCli));
  assert_eq!(builder.model.as_str(), "claude-opus-5-5");
  assert_eq!(notes.connection.as_deref(), Some("Codex main"));
  assert_eq!(notes.provider, Some(AgentProvider::CodexCli));
  assert_eq!(profile("Explorer").connection, None, "its connection is gone");
  assert!(!profile("Writer").mcp.ids().is_empty(), "Writer chooses an MCP server");
  assert!(
    assignables
      .profiles
      .iter()
      .all(|profile| &*profile.name != "Retired reviewer"),
    "an archived profile is no longer listed"
  );
  let command = |name: &str| {
    assignables
      .tools
      .iter()
      .find(|tool| &*tool.name == name)
      .and_then(|tool| tool.command.clone())
      .unwrap_or_else(|| panic!("{name} is a command tool"))
  };
  assert_eq!(&*command("Checks"), "npm test");
  assert!(command("Lint").len() > 60, "a command too long for the picker's row");
  let steps = opened.body.steps();
  assert_eq!(steps.len(), 8);
  let overrides: Vec<&str> = steps
    .iter()
    .filter_map(|step| match &step.kind {
      StepKind::Agent { model: Some(model), .. } => Some(model.as_str()),
      _ => None,
    })
    .collect();
  assert_eq!(overrides, ["claude-sonnet-5-5"], "one step overrides the model");
  let retired = match steps[7].kind {
    StepKind::Agent { profile: Some(id), .. } => id,
    _ => panic!("the last step runs a profile"),
  };
  assert!(
    assignables.profile(retired).is_none(),
    "the step names a profile the project no longer lists"
  );
}
