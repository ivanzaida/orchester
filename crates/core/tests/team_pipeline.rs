use std::collections::BTreeSet;

use orchester_core::{
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{
    DeliverMode, DeliverStep, MergeMethod, Pipeline, PromptText, RepositoryAccess, Step, StepKey, StepKind,
    TeamExecutors, TeamPipeline, TeamPipelineError,
  },
};

fn step(key: &str, kind: StepKind) -> Step {
  Step {
    key: StepKey::new(key).expect("valid key"),
    name: Name::new(key).expect("valid name"),
    kind,
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  }
}

fn agent(profile: Option<ResourceId>) -> StepKind {
  StepKind::Agent {
    profile,
    repository: RepositoryAccess::ReadOnly,
    model: None,
    permissions: StepPermissions::default(),
  }
}

fn pipeline(steps: Vec<Step>) -> Pipeline {
  Pipeline::new(steps).expect("a valid pipeline")
}

#[test]
fn a_pipeline_naming_team_profiles_and_tools_is_a_team_pipeline() {
  let (profile, tool) = (ResourceId::generate(), ResourceId::generate());
  let body = pipeline(vec![
    step("build", agent(Some(profile))),
    step("lint", StepKind::Tool { tool: Some(tool) }),
  ]);
  let team = TeamExecutors::new([profile], [tool]);

  let team_pipeline = TeamPipeline::new(body.clone(), &team).expect("every id is the team's");

  assert_eq!(team_pipeline.pipeline(), &body);
  assert_eq!(team_pipeline.into_pipeline(), body);
}

#[test]
fn a_step_naming_a_profile_the_team_does_not_have_is_refused() {
  let local = ResourceId::generate();
  let body = pipeline(vec![step("build", agent(Some(local)))]);
  let team = TeamExecutors::new([ResourceId::generate()], []);

  let refused = TeamPipeline::new(body, &team);

  assert_eq!(
    refused,
    Err(TeamPipelineError::Profile {
      step: StepKey::new("build").expect("valid key"),
      profile: local
    })
  );
}

#[test]
fn a_step_naming_a_tool_the_team_does_not_have_is_refused() {
  let local = ResourceId::generate();
  let body = pipeline(vec![step("lint", StepKind::Tool { tool: Some(local) })]);

  let refused = TeamPipeline::new(body, &TeamExecutors::default());

  assert_eq!(
    refused,
    Err(TeamPipelineError::Tool {
      step: StepKey::new("lint").expect("valid key"),
      tool: local
    })
  );
}

#[test]
fn a_tool_is_not_a_profile_and_a_profile_is_not_a_tool() {
  let id = ResourceId::generate();
  let as_profile = pipeline(vec![step("build", agent(Some(id)))]);
  let as_tool = pipeline(vec![step("lint", StepKind::Tool { tool: Some(id) })]);

  assert!(TeamPipeline::new(as_profile, &TeamExecutors::new([], [id])).is_err());
  assert!(TeamPipeline::new(as_tool, &TeamExecutors::new([id], [])).is_err());
}

#[test]
fn the_integrator_of_the_deliver_step_must_be_a_team_profile() {
  let profile = ResourceId::generate();
  let integrator = ResourceId::generate();
  let deliver = DeliverStep::new(DeliverMode::AutoMerge, MergeMethod::Squash, Some(integrator), true);
  let body = pipeline(vec![step("build", agent(Some(profile)))]).with_deliver(Some(deliver));

  let refused = TeamPipeline::new(body.clone(), &TeamExecutors::new([profile], []));
  assert_eq!(refused, Err(TeamPipelineError::Integrator { profile: integrator }));

  let accepted = TeamPipeline::new(body, &TeamExecutors::new([profile, integrator], []));
  assert!(accepted.is_ok());
}

#[test]
fn steps_without_a_profile_or_tool_yet_and_a_deliver_without_an_integrator_name_nothing() {
  let body = pipeline(vec![
    step("build", agent(None)),
    step("lint", StepKind::Tool { tool: None }),
  ])
  .with_deliver(Some(DeliverStep::default()));

  assert!(TeamPipeline::new(body, &TeamExecutors::default()).is_ok());
}

#[test]
fn a_team_pipeline_serializes_as_the_pipeline_body_the_server_reads() {
  let (profile, tool) = (ResourceId::generate(), ResourceId::generate());
  let body = pipeline(vec![
    step("build", agent(Some(profile))),
    step("lint", StepKind::Tool { tool: Some(tool) }),
  ]);
  let team = TeamExecutors::new([profile], [tool]);

  let value = serde_json::to_value(TeamPipeline::new(body, &team).expect("team ids").pipeline()).expect("a body");

  assert_eq!(value["steps"][0]["kind"]["agent"]["profile"], profile.to_string());
  assert_eq!(value["steps"][1]["kind"]["tool"]["tool"], tool.to_string());
}
