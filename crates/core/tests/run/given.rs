//! What a local run's agents are given (`docs/design/team-configuration.md`, "Decided 2026-11-09"): the instructions,
//! skills and tools their profile version chooses, whatever the bodies' scopes, targets or callers say; a scope still
//! decides where an instruction sits in the prompt, and its events when it is injected.

use std::collections::BTreeMap;

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{
    AgentProfile, AgentTargets, ChosenResources, Exclusion, InjectionEvent, Instruction, InstructionScope, Pipeline,
    PromptText, Skill, Version,
  },
  run::{GivenBy, InstructionSource, InstructionVersion, PromptPart, PromptSource, PublishedVersions, RunSnapshot},
};

use crate::{
  fixture::{Config, agent_step, key, profile, task, version},
  prompt::{Prompt, builder, step_with},
};

fn instruction(text: &str, targets: AgentTargets, events: &[InjectionEvent], scope: InstructionScope) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("valid prompt"),
    targets,
    events.iter().copied().collect(),
    scope,
  )
  .expect("valid instruction")
}

fn chosen(ids: &[ResourceId]) -> ChosenResources {
  ChosenResources::new(ids.to_vec()).expect("distinct")
}

fn source<'a>(resource: ResourceId, body: &'a Instruction) -> InstructionSource<'a> {
  InstructionSource {
    resource,
    name: "Rule",
    version: InstructionVersion::Published {
      number: orchester_core::resource::VersionNumber::FIRST,
    },
    body,
  }
}

#[test]
fn given_a_local_profile_gets_the_instructions_it_chooses_whatever_their_scope_and_targets() {
  let ids: Vec<ResourceId> = (0..4).map(|_| ResourceId::generate()).collect();
  let reviewer = Name::new("Reviewer").expect("valid role");
  let profile = builder("Role.").with_given(
    chosen(&[ids[0], ids[1], ids[3]]),
    ChosenResources::EMPTY,
    ChosenResources::EMPTY,
  );
  let mut prompt = Prompt::new(profile, step_with("Step."), task("Brief.", &[]));
  prompt.given = GivenBy::Profile;
  let start = [InjectionEvent::Start];
  let bodies = [
    instruction(
      "For reviewers only.",
      AgentTargets::All,
      &start,
      InstructionScope::AgentRole { role: reviewer },
    ),
    instruction(
      "Another pipeline's step.",
      AgentTargets::All,
      &start,
      InstructionScope::Step {
        pipeline: ResourceId::generate(),
        step: key("review"),
      },
    ),
    instruction("Not chosen.", AgentTargets::All, &start, InstructionScope::Project),
    instruction(
      "On resume.",
      AgentTargets::All,
      &[InjectionEvent::Resume],
      InstructionScope::Project,
    ),
  ];
  let sources: Vec<InstructionSource<'_>> = ids.iter().zip(&bodies).map(|(id, body)| source(*id, body)).collect();
  let composition = prompt.input(&sources, &[], true).compose(InjectionEvent::Start);

  let reasons: Vec<(ResourceId, Exclusion)> = composition
    .excluded()
    .iter()
    .map(|excluded| (excluded.resource, excluded.reason))
    .collect();
  assert_eq!(
    reasons,
    [(ids[2], Exclusion::NotChosen), (ids[3], Exclusion::OtherEvent)]
  );
  let parts: Vec<(ResourceId, PromptPart)> = composition
    .sources()
    .sources
    .iter()
    .filter_map(|source| match source {
      PromptSource::Instruction { resource, part, .. } => Some((*resource, *part)),
      _ => None,
    })
    .collect();
  assert_eq!(parts, [(ids[0], PromptPart::AgentRole), (ids[1], PromptPart::Step)]);
}

#[test]
fn given_a_run_pins_what_the_profiles_of_its_steps_choose_and_nothing_else() {
  let mut config = Config::feature_delivery();
  let (rule, skill, other, unpublished) = (
    ResourceId::generate(),
    ResourceId::generate(),
    ResourceId::generate(),
    ResourceId::generate(),
  );
  let builder = config.profiles[&config.builder].body().clone().with_given(
    chosen(&[rule, unpublished]),
    chosen(&[skill]),
    ChosenResources::EMPTY,
  );
  config.profiles.insert(config.builder, version(2, builder));
  let project = instruction(
    "Everyone.",
    AgentTargets::All,
    &[InjectionEvent::Start],
    InstructionScope::Project,
  );
  let instructions = BTreeMap::from([(rule, version(1, project.clone())), (other, version(1, project))]);
  let skills = BTreeMap::from([(
    skill,
    version(
      1,
      Skill::new("When.".into(), "How.".into(), Vec::new(), InstructionScope::Project).expect("skill"),
    ),
  )]);
  let published = PublishedVersions {
    profiles: &config.profiles,
    tools: &BTreeMap::new(),
    instructions: &instructions,
    skills: &skills,
  };
  let snapshot = RunSnapshot::build(config.pipeline, &config.pipeline_version, published).expect("snapshot");

  let pinned = |list: &[orchester_core::run::PinnedVersion]| list.iter().map(|pin| pin.resource).collect::<Vec<_>>();
  assert_eq!(pinned(snapshot.instructions()), [rule]);
  assert_eq!(pinned(snapshot.skills()), [skill]);
  assert!(snapshot.tools().is_empty());
}

#[test]
fn given_a_profile_that_runs_no_step_of_the_pipeline_pins_nothing() {
  let config = Config::feature_delivery();
  let idle = ResourceId::generate();
  let rule = ResourceId::generate();
  let mut profiles = config.profiles.clone();
  let chooser: AgentProfile =
    profile("Idle", "model-b").with_given(chosen(&[rule]), ChosenResources::EMPTY, ChosenResources::EMPTY);
  profiles.insert(idle, version(1, chooser));
  let body = instruction(
    "Idle.",
    AgentTargets::All,
    &[InjectionEvent::Start],
    InstructionScope::Project,
  );
  let instructions: BTreeMap<ResourceId, Version<Instruction>> = BTreeMap::from([(rule, version(1, body))]);
  let pipeline = Pipeline::new(vec![agent_step("implement", config.builder, &[])]).expect("pipeline");
  let published = PublishedVersions {
    profiles: &profiles,
    tools: &BTreeMap::new(),
    instructions: &instructions,
    skills: &BTreeMap::new(),
  };
  let snapshot = RunSnapshot::build(config.pipeline, &version(5, pipeline), published).expect("snapshot");
  assert!(snapshot.instructions().is_empty());
}
