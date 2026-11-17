//! A team project's instructions, skills and tools through the tools, so through the Orchestrator chat: the list and
//! get tools read the team's resources from its configuration, a save publishes the next version on the team server at
//! once, made on the version read, a new one is created there as v1, and the publish tools answer the resource as it
//! stands. What reaches agents' prompts, and what a tool runs, is a person's decision as in a local project.

use std::{collections::BTreeSet, sync::Arc};

use orchester_core::{
  id::ResourceId,
  resource::{
    AgentTargets, CommandAction, CommandProgram, InjectionEvent, Instruction, InstructionScope, PromptText, Skill,
    TeamTool, TeamToolAction, Tool, ToolAction,
  },
};
use parking_lot::Mutex;
use serde_json::{Value, json};

use super::{
  mcp_servers::{Answers, Seen, start},
  readiness::team_resource,
};
use crate::{Checkout, Standing, team_server::TeamServer};

/// The team's resources the tests change: an instruction, a skill and a command tool, each at v1.
struct Ids {
  instruction: ResourceId,
  skill: ResourceId,
  tool: ResourceId,
}

fn program() -> String {
  std::env::current_exe().expect("the test program").display().to_string()
}

/// A team with the instruction `house-style`, the skill `checks` and the command tool `build`, on a computer signed in
/// as an owner; `widens` is whether a person lets automation change agent permissions.
fn team(widens: bool) -> (TeamServer, Arc<Mutex<Seen>>, Checkout, Ids) {
  let (server, seen) = start(Answers::default());
  let mut ids = None;
  let owner = Standing { role: "owner", widens };
  let checkout = Checkout::build(
    Some(&server),
    (&[], owner),
    |db| {
      let rule = Instruction::new(
        PromptText::new("Use the house style.").expect("prompt"),
        AgentTargets::All,
        BTreeSet::from([InjectionEvent::Start]),
        InstructionScope::Project,
      )
      .expect("an instruction");
      let skill = Skill::new(
        "Checks the change.".to_owned(),
        "Look.".to_owned(),
        Vec::new(),
        InstructionScope::Project,
      )
      .expect("a skill");
      let command = CommandAction::new(CommandProgram::new(&program()).expect("program"), Vec::new());
      let team_tool = TeamTool::new("Builds.", TeamToolAction::Command(command.clone()), None).expect("a tool");
      let tool = Tool::new("Builds.", ToolAction::Command(command), None).expect("a tool");
      ids = Some(Ids {
        instruction: team_resource(db, "house-style", json!(rule), &rule),
        skill: team_resource(db, "checks", json!(skill), &skill),
        tool: team_resource(db, "build", json!(team_tool), &tool),
      });
    },
    true,
  );
  (server, seen, checkout, ids.expect("prepared"))
}

fn kontur(mut args: Value) -> Value {
  args["project"] = json!("Kontur");
  args
}

/// Each kind's tools: list, get, save, publish, the argument that names one and the key of the answer.
const KINDS: [(&str, &str, &str, &str, &str, &str); 3] = [
  (
    "list_instructions",
    "get_instruction",
    "save_instruction_draft",
    "publish_instruction",
    "instruction",
    "instructions",
  ),
  (
    "list_skills",
    "get_skill",
    "save_skill_draft",
    "publish_skill",
    "skill",
    "skills",
  ),
  (
    "list_tools",
    "get_tool",
    "save_tool_draft",
    "publish_tool",
    "tool",
    "tools",
  ),
];

#[test]
fn team_mcp_resources_the_teams_instructions_skills_and_tools_are_listed_and_opened() {
  let (_server, _seen, checkout, ids) = team(false);

  for ((list, get, .., key, plural), (name, id)) in KINDS.into_iter().zip([
    ("house-style", ids.instruction),
    ("checks", ids.skill),
    ("build", ids.tool),
  ]) {
    let opened = checkout
      .call(get, &kontur(json!({ key: name })))
      .expect("opened by its name");
    assert_eq!(opened[key]["id"], id.to_string(), "{opened}");
    assert_eq!(opened[key]["team"], true, "{opened}");
    assert_eq!(opened[key]["revision"], "1", "{opened}");

    let listed = checkout.call(list, &kontur(json!({}))).expect("listed");
    let found: Vec<&Value> = listed[plural]
      .as_array()
      .expect("listed")
      .iter()
      .filter(|entry| entry["id"] == id.to_string())
      .collect();
    assert_eq!(found.len(), 1, "listed once: {listed}");
    assert_eq!(found[0]["team"], true, "{listed}");
  }
  let tool = checkout
    .call("get_tool", &kontur(json!({ "tool": "build" })))
    .expect("opened");
  assert_eq!(tool["tool"]["command"]["program"], program(), "{tool}");
}

#[test]
fn team_mcp_resources_a_save_publishes_the_next_version_and_publish_answers_it_as_it_stands() {
  let (_server, seen, checkout, ids) = team(true);
  let saves = [
    json!({ "instruction": ids.instruction.to_string(), "events": "start", "text": "Use the house style, always." }),
    json!({ "skill": ids.skill.to_string(), "body": "Look twice.", "expected_revision": "1" }),
    json!({ "tool": ids.tool.to_string(), "description": "Builds the change.", "expected_revision": "1" }),
  ];

  for ((.., save, publish, key, _), (args, id)) in
    KINDS
      .into_iter()
      .zip(saves.into_iter().zip([ids.instruction, ids.skill, ids.tool]))
  {
    let saved = checkout.call(save, &kontur(args)).expect("published");
    assert_eq!(saved[key]["revision"], "2", "{save}: {saved}");
    let answered = checkout
      .call(publish, &kontur(json!({ key: id.to_string() })))
      .expect("answered");
    assert_eq!(answered[key]["revision"], "2", "{publish}: {answered}");
  }
  let seen = seen.lock();
  assert_eq!(
    seen.calls,
    ["publish", "publish", "publish"],
    "no draft, nothing else sent"
  );
  assert!(seen.published.iter().all(|published| published["based_on"] == 1));
  assert_eq!(seen.published[0]["body"]["text"], "Use the house style, always.");
  assert_eq!(seen.published[1]["body"]["body"], "Look twice.");
  assert_eq!(seen.published[2]["body"]["description"], "Builds the change.");
}

#[test]
fn team_mcp_resources_a_save_made_on_an_older_version_is_refused_and_nothing_is_sent() {
  let (_server, seen, checkout, ids) = team(true);

  let skill = json!({ "skill": ids.skill.to_string(), "body": "Look twice.", "expected_revision": "7" });
  assert_eq!(checkout.refused("save_skill_draft", &kontur(skill)), "draft_changed");
  let tool = json!({ "tool": ids.tool.to_string(), "description": "Builds.", "expected_revision": "7" });
  assert_eq!(checkout.refused("save_tool_draft", &kontur(tool)), "draft_changed");
  let renamed = json!({ "skill": ids.skill.to_string(), "name": "Other" });
  assert_eq!(
    checkout.refused("save_skill_draft", &kontur(renamed)),
    "name_not_editable"
  );

  assert!(seen.lock().calls.is_empty(), "nothing was sent");
}

#[test]
fn team_mcp_resources_a_new_one_is_created_on_the_team_server_as_v1() {
  let (_server, seen, checkout, _) = team(true);
  let made = [
    json!({ "name": "Tests first", "events": "start", "text": "Write the failing test first." }),
    json!({ "name": "Review", "description": "Reviews the change.", "body": "Read it." }),
    json!({ "name": "Lint", "program": program() }),
  ];

  for ((.., save, _, key, _), args) in KINDS.into_iter().zip(made) {
    let created = checkout.call(save, &kontur(args)).expect("created");
    assert_eq!(created[key]["revision"], "1", "{save}: {created}");
  }
  let taken = json!({ "name": "build", "program": program() });
  assert_eq!(checkout.refused("save_tool_draft", &kontur(taken)), "name_taken");

  let seen = seen.lock();
  assert_eq!(seen.calls, ["create", "create", "create"]);
  let kinds: Vec<&str> = seen.created.iter().filter_map(|made| made["kind"].as_str()).collect();
  assert_eq!(kinds, ["instruction", "skill", "tool"]);
}

#[test]
fn team_mcp_resources_a_prompt_change_is_a_persons_while_automation_may_not_change_agent_permissions() {
  let (_server, seen, checkout, ids) = team(false);

  let rule = json!({ "instruction": ids.instruction.to_string(), "events": "start", "text": "Skip the style." });
  assert_eq!(checkout.refused("save_instruction_draft", &kontur(rule)), "person_only");
  let skill = json!({ "skill": ids.skill.to_string(), "body": "Skip the checks." });
  assert_eq!(checkout.refused("save_skill_draft", &kontur(skill)), "person_only");
  let widened = json!({ "tool": ids.tool.to_string(), "arguments": ["--everything"] });
  assert_eq!(checkout.refused("save_tool_draft", &kontur(widened)), "person_only");
  let new_tool = json!({ "name": "Lint", "program": program() });
  assert_eq!(checkout.refused("save_tool_draft", &kontur(new_tool)), "person_only");
  assert!(seen.lock().calls.is_empty(), "nothing was sent");

  // A tool's description changes nothing it runs: it is published.
  let described = json!({ "tool": ids.tool.to_string(), "description": "Builds the change." });
  checkout.call("save_tool_draft", &kontur(described)).expect("published");
  assert_eq!(seen.lock().calls, ["publish"]);
}
