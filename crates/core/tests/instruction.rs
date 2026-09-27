use std::collections::BTreeSet;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    AgentTargets, Exclusion, InjectionEvent, Instruction, InstructionError, InstructionScope, InstructionTarget,
    PromptText, PublishError, ResourceBody, StepKey,
  },
};

fn text(value: &str) -> PromptText {
  PromptText::new(value).expect("valid prompt")
}

fn roles(names: &[&str]) -> BTreeSet<Name> {
  names.iter().map(|role| Name::new(role).expect("valid role")).collect()
}

fn start_and_resume() -> BTreeSet<InjectionEvent> {
  BTreeSet::from([InjectionEvent::Start, InjectionEvent::Resume])
}

#[test]
fn instruction_targets_selected_roles_at_selected_events() {
  let instruction = Instruction::new(
    text("Work on {{run.branch}} only."),
    AgentTargets::Roles(roles(&["Builder", "Reviewer"])),
    start_and_resume(),
    InstructionScope::Project,
  )
  .unwrap();
  assert_eq!(
    instruction.targets(),
    &AgentTargets::Roles(roles(&["Builder", "Reviewer"]))
  );
  assert_eq!(instruction.events(), &start_and_resume());
  assert_eq!(instruction.scope(), &InstructionScope::Project);
}

#[test]
fn instruction_scopes_cover_role_pipeline_and_step() {
  let pipeline = ResourceId::generate();
  let scopes = [
    InstructionScope::AgentRole {
      role: Name::new("Builder").unwrap(),
    },
    InstructionScope::Pipeline { pipeline },
    InstructionScope::Step {
      pipeline,
      step: StepKey::new("implement").unwrap(),
    },
  ];
  for scope in scopes {
    let instruction = Instruction::new(text("Rule."), AgentTargets::All, start_and_resume(), scope.clone()).unwrap();
    assert_eq!(instruction.scope(), &scope);
  }
}

#[test]
fn instruction_draft_may_be_blank_but_publishing_needs_text() {
  let blank = Instruction::new(
    text(
      "  
",
    ),
    AgentTargets::All,
    start_and_resume(),
    InstructionScope::Project,
  )
  .expect("a new draft starts without text");
  assert_eq!(blank.check_publishable(), Err(PublishError::BlankInstruction));
  let written = Instruction::new(
    text("Rule."),
    AgentTargets::All,
    start_and_resume(),
    InstructionScope::Project,
  )
  .unwrap();
  assert_eq!(written.check_publishable(), Ok(()));
}

#[test]
fn instruction_rejects_an_empty_role_or_profile_selection() {
  for targets in [
    AgentTargets::Roles(BTreeSet::new()),
    AgentTargets::Profiles(BTreeSet::new()),
  ] {
    assert_eq!(
      Instruction::new(text("Rule."), targets, start_and_resume(), InstructionScope::Project),
      Err(InstructionError::NoTargets)
    );
  }
}

#[test]
fn instruction_needs_an_injection_event() {
  assert_eq!(
    Instruction::new(
      text("Rule."),
      AgentTargets::All,
      BTreeSet::new(),
      InstructionScope::Project
    ),
    Err(InstructionError::NoEvents)
  );
}

#[test]
fn instruction_for_a_role_cannot_select_other_agents() {
  let scope = InstructionScope::AgentRole {
    role: Name::new("Builder").unwrap(),
  };
  for targets in [
    AgentTargets::Roles(roles(&["Reviewer"])),
    AgentTargets::Profiles(BTreeSet::from([ResourceId::generate()])),
  ] {
    assert_eq!(
      Instruction::new(text("Rule."), targets, start_and_resume(), scope.clone()),
      Err(InstructionError::RoleScopeSelectsAgents)
    );
  }
}

/// Implement of `pipeline`, run by the Builder profile `profile`.
fn implement_target<'a>(
  pipeline: ResourceId,
  profile: ResourceId,
  step: &'a StepKey,
  role: &'a Name,
) -> InstructionTarget<'a> {
  InstructionTarget {
    pipeline,
    step,
    profile,
    role,
  }
}

fn rule(targets: AgentTargets, events: &[InjectionEvent], scope: InstructionScope) -> Instruction {
  Instruction::new(text("Rule."), targets, events.iter().copied().collect(), scope).expect("valid instruction")
}

#[test]
fn instruction_reaches_selected_agents_at_its_events() {
  let (pipeline, profile) = (ResourceId::generate(), ResourceId::generate());
  let (implement, builder) = (StepKey::new("implement").unwrap(), Name::new("Builder").unwrap());
  let target = implement_target(pipeline, profile, &implement, &builder);
  let start = [InjectionEvent::Start];
  let cases = [
    (AgentTargets::All, &start[..], Ok(())),
    (AgentTargets::Roles(roles(&["Builder"])), &start[..], Ok(())),
    (
      AgentTargets::Roles(roles(&["Reviewer"])),
      &start[..],
      Err(Exclusion::OtherAgents),
    ),
    (AgentTargets::Profiles(BTreeSet::from([profile])), &start[..], Ok(())),
    (
      AgentTargets::Profiles(BTreeSet::from([ResourceId::generate()])),
      &start[..],
      Err(Exclusion::OtherAgents),
    ),
    (
      AgentTargets::All,
      &[InjectionEvent::Resume][..],
      Err(Exclusion::OtherEvent),
    ),
  ];
  for (targets, events, expected) in cases {
    let instruction = rule(targets, events, InstructionScope::Project);
    assert_eq!(
      instruction.applies(target, InjectionEvent::Start),
      expected,
      "{instruction:?}"
    );
  }
}

#[test]
fn instruction_reaches_agents_of_its_role_pipeline_or_step() {
  let (pipeline, profile) = (ResourceId::generate(), ResourceId::generate());
  let (implement, builder) = (StepKey::new("implement").unwrap(), Name::new("Builder").unwrap());
  let target = implement_target(pipeline, profile, &implement, &builder);
  let reviewer = Name::new("Reviewer").unwrap();
  let cases = [
    (InstructionScope::AgentRole { role: builder.clone() }, Ok(())),
    (
      InstructionScope::AgentRole { role: reviewer },
      Err(Exclusion::OtherRole),
    ),
    (InstructionScope::Pipeline { pipeline }, Ok(())),
    (
      InstructionScope::Pipeline {
        pipeline: ResourceId::generate(),
      },
      Err(Exclusion::OtherPipeline),
    ),
    (
      InstructionScope::Step {
        pipeline,
        step: implement.clone(),
      },
      Ok(()),
    ),
    (
      InstructionScope::Step {
        pipeline,
        step: StepKey::new("review").unwrap(),
      },
      Err(Exclusion::OtherStep),
    ),
  ];
  for (scope, expected) in cases {
    let instruction = rule(AgentTargets::All, &[InjectionEvent::Start], scope);
    assert_eq!(instruction.reaches(target), expected, "{instruction:?}");
  }
}

#[test]
fn instruction_body_round_trips_and_rejects_unknown_variables_on_read() {
  let instruction = Instruction::new(
    text("Summarize {{task.title}}."),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .unwrap();
  let json = serde_json::to_string(&instruction).unwrap();
  assert_eq!(serde_json::from_str::<Instruction>(&json).unwrap(), instruction);

  let unknown = json.replace("task.title", "task.owner");
  let error = serde_json::from_str::<Instruction>(&unknown).unwrap_err();
  assert!(error.to_string().contains("task.owner"), "{error}");

  let no_events = json.replace(r#"["start"]"#, "[]");
  assert!(serde_json::from_str::<Instruction>(&no_events).is_err());
}
