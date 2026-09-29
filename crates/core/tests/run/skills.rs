//! Skills in a run: the snapshot pins the published skills whose scope reaches an agent of the
//! pipeline, and a new session's prompt lists the skills its agent gets before the task part.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{InjectionEvent, InstructionScope, Skill},
  run::{PublishedVersions, RunSnapshot, SkillListing},
};

use crate::{
  fixture::{Config, key, task, version},
  prompt::{Prompt, builder, step_with},
};

fn skill(scope: InstructionScope) -> Skill {
  Skill::new("Runs the checks.".into(), "Run `cargo test`.".into(), Vec::new(), scope).expect("valid skill")
}

#[test]
fn run_snapshot_pins_the_skills_whose_scope_reaches_an_agent_of_the_pipeline() {
  let config = Config::feature_delivery();
  let role = |name: &str| InstructionScope::AgentRole {
    role: Name::new(name).expect("valid"),
  };
  let scoped = [
    (InstructionScope::Project, true),
    (role("Reviewer"), true),
    (role("Designer"), false),
    (
      InstructionScope::Pipeline {
        pipeline: config.pipeline,
      },
      true,
    ),
    (
      InstructionScope::Pipeline {
        pipeline: ResourceId::generate(),
      },
      false,
    ),
    (
      InstructionScope::Step {
        pipeline: config.pipeline,
        step: key("checks"),
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
  let skills: BTreeMap<_, _> = scoped
    .iter()
    .map(|(scope, _)| (ResourceId::generate(), version(1, skill(scope.clone()))))
    .collect();
  let (tools, instructions) = (BTreeMap::new(), BTreeMap::new());
  let published = PublishedVersions {
    profiles: &config.profiles,
    tools: &tools,
    instructions: &instructions,
    skills: &skills,
  };
  let snapshot = RunSnapshot::build(config.pipeline, &config.pipeline_version, published).expect("published");
  let pinned: BTreeSet<ResourceId> = snapshot.skills().iter().map(|pin| pin.resource).collect();
  let expected: BTreeSet<ResourceId> = skills
    .iter()
    .filter(|(_, version)| {
      scoped
        .iter()
        .any(|(scope, reaches)| *reaches && version.body().scope() == scope)
    })
    .map(|(id, _)| *id)
    .collect();
  assert_eq!(pinned, expected);
  assert_eq!(pinned.len(), 4);
}

#[test]
fn run_snapshot_of_a_run_created_before_skills_reads_with_none() {
  let config = Config::feature_delivery();
  let mut stored = serde_json::to_value(config.snapshot()).expect("serializes");
  stored.as_object_mut().expect("an object").remove("skills");
  let read: RunSnapshot = serde_json::from_value(stored).expect("an older snapshot reads");
  assert!(read.skills().is_empty());
}

#[test]
fn new_session_prompt_lists_each_skill_before_the_task_and_a_resumed_one_does_not() {
  let prompt = Prompt::new(builder("Fine."), step_with("Implement it."), task("Brief.", &[]));
  let listings = [
    SkillListing {
      name: "run-checks",
      description: "Runs the checks\n  before a pull request.",
      path: "/repo/.orchester/skills/e1/run-checks/SKILL.md",
    },
    SkillListing {
      name: "write-migration",
      description: "Writes a migration.",
      path: "/repo/.orchester/skills/e1/write-migration/SKILL.md",
    },
  ];
  let mut input = prompt.input(&[], &[], true);
  input.skills = &listings;
  let text = input.assemble(InjectionEvent::Start).expect("assembles").text;
  let skills = text.find("# Skills\n").expect("a skills part");
  let task_part = text.find("# Task ORC-12").expect("a task part");
  assert!(text.find("# Current step").expect("a step part") < skills);
  assert!(skills < task_part);
  assert!(text.contains(
    "- run-checks: Runs the checks before a pull request. (/repo/.orchester/skills/e1/run-checks/SKILL.md)\n- \
     write-migration: Writes a migration. (/repo/.orchester/skills/e1/write-migration/SKILL.md)\n\n# Task ORC-12"
  ));
  assert!(text.contains("read its SKILL.md first"));

  let resumed = input.assemble(InjectionEvent::Resume).expect("assembles").text;
  assert!(!resumed.contains("# Skills"));
}

#[test]
fn prompt_without_skills_has_no_skills_part() {
  let prompt = Prompt::new(builder("Fine."), step_with("Implement it."), task("Brief.", &[]));
  let text = prompt
    .input(&[], &[], true)
    .assemble(InjectionEvent::Start)
    .expect("assembles")
    .text;
  assert!(!text.contains("# Skills"));
}
