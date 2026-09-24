use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    AccessPolicy, AccessWindow, AgentTargets, AllowedSteps, ApprovalMode, CommandAction, DeniedBehavior,
    InjectionEvent, Instruction, InstructionScope, Pipeline, PromptText, Step, StepKind, Tool, ToolAction,
  },
  revision::Revision,
  run::{BranchName, PinnedVersion, PublishedVersions, RunSnapshot, SnapshotError, StepExecutor},
};

use crate::fixture::{Config, agent_step, key, model, version};

fn instruction(scope: InstructionScope) -> Instruction {
  Instruction::new(
    PromptText::new("Rule.").expect("valid prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    scope,
  )
  .expect("valid instruction")
}

fn tool(callers: BTreeMap<ResourceId, AllowedSteps>) -> Tool {
  let access = AccessPolicy::new(
    callers,
    Name::new("Staging").expect("valid name"),
    ApprovalMode::Never,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("valid policy");
  let action = CommandAction::new("cargo".into(), vec!["test".into()]).expect("valid command");
  Tool::new(ToolAction::Command(action), access)
}

fn build(
  config: &Config,
  pipeline: &Pipeline,
  tools: &BTreeMap<ResourceId, orchester_core::resource::Version<Tool>>,
) -> Result<RunSnapshot, SnapshotError> {
  let instructions = BTreeMap::new();
  let published = PublishedVersions {
    profiles: &config.profiles,
    tools,
    instructions: &instructions,
  };
  RunSnapshot::build(config.pipeline, &version(4, pipeline.clone()), published)
}

#[test]
fn run_snapshot_pins_pipeline_and_profile_versions_with_the_profile_model() {
  let config = Config::feature_delivery();
  let snapshot = config.snapshot();
  assert_eq!(
    snapshot.pipeline(),
    PinnedVersion::of(config.pipeline, &config.pipeline_version)
  );
  assert_eq!(snapshot.body(), config.pipeline_version.body());
  let implement = snapshot.step(&key("implement")).expect("step");
  assert_eq!(
    implement.executor(),
    &StepExecutor::Agent {
      profile: PinnedVersion::of(config.builder, &config.profiles[&config.builder]),
      model: model("model-b"),
      model_overridden: false,
    }
  );
}

#[test]
fn run_snapshot_step_model_override_wins_over_the_profile_model() {
  let config = Config::feature_delivery();
  let mut steps = config.pipeline_version.body().steps().to_vec();
  let StepKind::Agent { model: step_model, .. } = &mut steps[2].kind else {
    panic!("implement is an agent step");
  };
  *step_model = Some(model("model-override"));
  let pipeline = Pipeline::new(steps).expect("valid pipeline");
  let snapshot = build(&config, &pipeline, &BTreeMap::new()).expect("published");
  assert_eq!(
    snapshot.step(&key("implement")).map(|step| step.executor()),
    Some(&StepExecutor::Agent {
      profile: PinnedVersion::of(config.builder, &config.profiles[&config.builder]),
      model: model("model-override"),
      model_overridden: true,
    })
  );
  assert!(matches!(
    snapshot.step(&key("research")).map(|step| step.executor()),
    Some(StepExecutor::Agent {
      model_overridden: false,
      ..
    })
  ));
}

#[test]
fn run_snapshot_rejects_unpublished_profiles_and_tools() {
  let config = Config::feature_delivery();
  let missing_profile = ResourceId::generate();
  let pipeline = Pipeline::new(vec![agent_step("research", missing_profile, &[])]).expect("valid");
  assert_eq!(
    build(&config, &pipeline, &BTreeMap::new()),
    Err(SnapshotError::UnpublishedProfile {
      step: key("research"),
      profile: missing_profile,
    })
  );
  let missing_tool = ResourceId::generate();
  let tool_step = Step {
    kind: StepKind::Tool {
      tool: Some(missing_tool),
    },
    ..agent_step("deploy", config.builder, &[])
  };
  let pipeline = Pipeline::new(vec![tool_step]).expect("valid");
  assert_eq!(
    build(&config, &pipeline, &BTreeMap::new()),
    Err(SnapshotError::UnpublishedTool {
      step: key("deploy"),
      tool: missing_tool,
    })
  );
}

#[test]
fn run_snapshot_rejects_steps_without_a_profile_or_tool() {
  let config = Config::feature_delivery();
  let unassigned = Step {
    kind: StepKind::Tool { tool: None },
    ..agent_step("deploy", config.builder, &[])
  };
  let pipeline = Pipeline::new(vec![unassigned]).expect("valid");
  assert_eq!(
    build(&config, &pipeline, &BTreeMap::new()),
    Err(SnapshotError::UnassignedStep(key("deploy")))
  );
}

#[test]
fn run_snapshot_pins_applicable_instructions_only() {
  let config = Config::feature_delivery();
  let other_pipeline = ResourceId::generate();
  let scoped = [
    (InstructionScope::Project, true),
    (
      InstructionScope::AgentRole {
        role: Name::new("Builder").expect("valid"),
      },
      true,
    ),
    (
      InstructionScope::AgentRole {
        role: Name::new("Designer").expect("valid"),
      },
      false,
    ),
    (
      InstructionScope::Pipeline {
        pipeline: config.pipeline,
      },
      true,
    ),
    (
      InstructionScope::Pipeline {
        pipeline: other_pipeline,
      },
      false,
    ),
    (
      InstructionScope::Step {
        pipeline: config.pipeline,
        step: key("review"),
      },
      true,
    ),
    (
      InstructionScope::Step {
        pipeline: config.pipeline,
        step: key("deploy"),
      },
      false,
    ),
  ];
  let ids: Vec<(ResourceId, bool)> = scoped
    .iter()
    .map(|(_, applies)| (ResourceId::generate(), *applies))
    .collect();
  let instructions: BTreeMap<_, _> = ids
    .iter()
    .zip(scoped)
    .map(|((id, _), (scope, _))| (*id, version(1, instruction(scope))))
    .collect();
  let tools = BTreeMap::new();
  let published = PublishedVersions {
    profiles: &config.profiles,
    tools: &tools,
    instructions: &instructions,
  };
  let snapshot = RunSnapshot::build(config.pipeline, &config.pipeline_version, published).expect("published");
  let pinned: BTreeSet<ResourceId> = snapshot.instructions().iter().map(|pin| pin.resource).collect();
  let expected: BTreeSet<ResourceId> = ids.iter().filter(|(_, applies)| *applies).map(|(id, _)| *id).collect();
  assert_eq!(pinned, expected);
}

#[test]
fn run_snapshot_pins_step_tools_and_tools_allowed_for_the_pipeline() {
  let config = Config::feature_delivery();
  let step_tool = ResourceId::generate();
  let allowed_tool = ResourceId::generate();
  let foreign_tool = ResourceId::generate();
  let tools = BTreeMap::from([
    (step_tool, version(1, tool(BTreeMap::new()))),
    (
      allowed_tool,
      version(
        2,
        tool(BTreeMap::from([(
          config.pipeline,
          AllowedSteps::Only(BTreeSet::from([key("checks")])),
        )])),
      ),
    ),
    (
      foreign_tool,
      version(1, tool(BTreeMap::from([(ResourceId::generate(), AllowedSteps::All)]))),
    ),
  ]);
  let mut steps = config.pipeline_version.body().steps().to_vec();
  steps.push(Step {
    kind: StepKind::Tool { tool: Some(step_tool) },
    ..agent_step("deploy", config.builder, &["pr"])
  });
  let pipeline = Pipeline::new(steps).expect("valid pipeline");
  let snapshot = build(&config, &pipeline, &tools).expect("published");
  let pinned: BTreeSet<_> = snapshot
    .tools()
    .iter()
    .copied()
    .map(|pin| (pin.resource, pin.number.get()))
    .collect();
  assert_eq!(pinned, BTreeSet::from([(step_tool, 1), (allowed_tool, 2)]));
}

#[test]
fn run_snapshot_round_trips_through_json_and_rejects_tampering() {
  let snapshot = Config::feature_delivery().snapshot();
  let json = serde_json::to_value(&snapshot).expect("serializable");
  assert_eq!(
    serde_json::from_value::<RunSnapshot>(json.clone()).expect("valid"),
    snapshot
  );
  let mut tampered = json.clone();
  tampered["steps"][2]["executor"]["profile"]["resource"] = serde_json::json!(ResourceId::generate());
  assert!(serde_json::from_value::<RunSnapshot>(tampered).is_err());
  let mut shortened = json;
  shortened["steps"].as_array_mut().expect("steps").pop();
  assert!(serde_json::from_value::<RunSnapshot>(shortened).is_err());
}

#[test]
fn run_snapshot_downstream_includes_transitive_dependents() {
  let snapshot = Config::feature_delivery().snapshot();
  let downstream: Vec<_> = snapshot
    .downstream_of(&key("implement"))
    .into_iter()
    .map(|k| k.as_str().to_owned())
    .collect();
  assert_eq!(downstream, ["checks", "implement", "pr", "review"]);
}

#[test]
fn run_snapshot_revisions_are_full_lowercase_shas() {
  assert!(Revision::new(&"0123456789abcdef".repeat(3)[..40]).is_ok());
  for invalid in ["", "4b92ad0", &"A".repeat(40), &"g".repeat(40), &"a".repeat(41)] {
    assert!(Revision::new(invalid).is_err(), "{invalid} was accepted");
  }
}

#[test]
fn run_snapshot_branch_names_follow_git_rules() {
  assert!(BranchName::new("orchester/orc-129-repo-switcher").is_ok());
  for invalid in [
    "", "-x", "a..b", "a b", "a~1", "x.lock", "a/", "/a", "a//b", "@", "a@{b", "a\\b", "a/.b",
  ] {
    assert!(BranchName::new(invalid).is_err(), "{invalid} was accepted");
  }
}
