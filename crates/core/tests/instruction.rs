use std::collections::BTreeSet;

use orchester_core::{
  id::ResourceId,
  resource::{AgentTargets, InjectionEvent, Instruction, InstructionError, InstructionScope, PromptText, StepKey},
};

fn text(value: &str) -> PromptText {
  PromptText::new(value).expect("valid prompt")
}

fn start_and_resume() -> BTreeSet<InjectionEvent> {
  BTreeSet::from([InjectionEvent::Start, InjectionEvent::Resume])
}

#[test]
fn instruction_targets_selected_profiles_at_selected_events() {
  let builder = ResourceId::generate();
  let instruction = Instruction::new(
    text("Work on {{run.branch}} only."),
    AgentTargets::Profiles(BTreeSet::from([builder])),
    start_and_resume(),
    InstructionScope::Project,
  )
  .unwrap();
  assert_eq!(
    instruction.targets(),
    &AgentTargets::Profiles(BTreeSet::from([builder]))
  );
  assert_eq!(instruction.events(), &start_and_resume());
  assert_eq!(instruction.scope(), &InstructionScope::Project);
}

#[test]
fn instruction_scopes_cover_role_pipeline_and_step() {
  let pipeline = ResourceId::generate();
  let scopes = [
    InstructionScope::AgentRole {
      profile: ResourceId::generate(),
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
fn instruction_rejects_blank_text() {
  assert_eq!(
    Instruction::new(
      text("  \n"),
      AgentTargets::All,
      start_and_resume(),
      InstructionScope::Project
    ),
    Err(InstructionError::BlankText)
  );
}

#[test]
fn instruction_rejects_an_empty_profile_selection() {
  assert_eq!(
    Instruction::new(
      text("Rule."),
      AgentTargets::Profiles(BTreeSet::new()),
      start_and_resume(),
      InstructionScope::Project
    ),
    Err(InstructionError::NoProfiles)
  );
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
fn instruction_for_a_role_cannot_target_other_profiles() {
  let scope = InstructionScope::AgentRole {
    profile: ResourceId::generate(),
  };
  let targets = AgentTargets::Profiles(BTreeSet::from([ResourceId::generate()]));
  assert_eq!(
    Instruction::new(text("Rule."), targets, start_and_resume(), scope),
    Err(InstructionError::RoleScopeTargetsProfiles)
  );
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
