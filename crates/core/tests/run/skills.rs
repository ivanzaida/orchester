//! Skills in a run: the snapshot pins the published skills its profiles choose (`given`), and a new session's prompt
//! lists the skills its agent gets before the task part. A skill a person chose for a message (ORC-634) reaches the
//! agent in its block with the message.

use std::num::NonZeroU32;

use orchester_core::{
  connection::invocation::Platform,
  conversation::{Message, MessageBody, Recipient, skill::ChosenSkill},
  id::{ResourceId, TaskId},
  resource::{InjectionEvent, InstructionScope, Skill, Slug, VersionNumber},
  run::{DeliveredMessages, MessageOccasion, NativeSetting, PromptInput, RunSnapshot, SkillListing},
};

use crate::{
  fixture::{Config, at, task},
  prompt::{Prompt, builder, step_with},
};

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
      tool: None,
    },
    SkillListing {
      name: "write-migration",
      description: "Writes a migration.",
      path: "/repo/.orchester/skills/e1/write-migration/SKILL.md",
      tool: None,
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

/// An agent that has Orchester's `read_skill` is told to read its skills with it by name, and no skill's path:
/// `read_outside_file` refuses the folder they were placed in. Only the listed skills are kept from being read by
/// path: the repository's own skill documents stay files the agent's file tools read. The text is the same for
/// Claude Code's and Codex's name of the tool.
#[test]
fn new_session_prompt_names_skills_for_read_skill_without_their_paths() {
  for tool in ["mcp__Orchester__read_skill", "read_skill"] {
    let prompt = Prompt::new(builder("Fine."), step_with("Implement it."), task("Brief.", &[]));
    let listings = [SkillListing {
      name: "run-checks",
      description: "Runs the checks.",
      path: "/repo/.orchester/skills/e1/run-checks/SKILL.md",
      tool: Some(tool),
    }];
    let mut input = prompt.input(&[], &[], true);
    input.skills = &listings;
    let text = input.assemble(InjectionEvent::Start).expect("assembles").text;
    assert!(
      text.contains(&format!(
        "Read a skill with the {tool} tool by its name, and a reference file with its name and the file's path in \
         the skill; do not read the skills listed here by path. Other files of the repository, skill documents in \
         its own folders included, are read with your usual file tools when your instructions point at them."
      )),
      "{text}"
    );
    assert!(!text.contains("do not read skills by path"), "{text}");
    assert!(text.contains("- run-checks: Runs the checks.\n"), "{text}");
    assert!(!text.contains(".orchester/skills"), "{text}");
  }
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

/// A person's message kept with the skill `release-notes` they chose for it, as the conversation keeps it.
fn message_with_skill() -> Message {
  let chosen = ChosenSkill {
    resource: ResourceId::generate(),
    slug: Slug::new("release-notes").expect("a slug"),
    version: VersionNumber::new(NonZeroU32::new(2).expect("not zero")),
  };
  let skill = Skill::new(
    "Release notes from the merged tasks.".into(),
    "List the tasks merged since the last tag.".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("a skill");
  let body = MessageBody::new(&chosen.kept_with(&skill, "for v0.4")).expect("a message");
  Message::from_person(TaskId::generate(), None, Recipient::Orchestrator, body, at(40))
}

/// ORC-634: a native agent reads the skill a person chose for a message in its block, with the message, in the first
/// user item of its conversation, and so does an agent CLI in its prompt: the skill's text is Orchester's, marked.
#[test]
fn a_chosen_skill_reaches_a_native_agent_and_a_cli_in_its_block_with_the_message() {
  let prompt = Prompt::new(builder("Fine."), step_with("Implement it."), task("Brief.", &[]));
  let messages = [message_with_skill()];
  let input = PromptInput {
    messages: Some(DeliveredMessages {
      occasion: MessageOccasion::Written,
      messages: &messages,
    }),
    ..prompt.input(&[], &[], true)
  };
  let native = input
    .assemble_native(NativeSetting {
      platform: Platform::Windows,
      shell: None,
      repository_instructions: None,
    })
    .expect("assembles");
  for (agent, text) in [
    ("native", native.task),
    ("cli", input.assemble(InjectionEvent::Start).expect("assembles").text),
  ] {
    let heading = text.find("# Messages from the person").expect("the messages");
    let block = text[heading..]
      .find("<orchester-skill name=\"release-notes\" version=\"2\"")
      .unwrap_or_else(|| panic!("{agent}: the skill's block: {text}"));
    let words = &text[heading + block..];
    assert!(
      words.contains("Orchester added this block: the person chose the skill release-notes"),
      "{agent}: {text}"
    );
    assert!(
      words.contains("List the tasks merged since the last tag."),
      "{agent}: {text}"
    );
    assert!(words.contains("</orchester-skill>\n\nfor v0.4"), "{agent}: {text}");
  }
}
