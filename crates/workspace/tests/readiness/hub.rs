//! The Agents hub of a team project (`Workspace::team_hub`): what each MCP server and tool is, the instructions and
//! skills that need nothing of a computer, and how the hub numbers and orders them.

use std::collections::BTreeSet;

use orchester_core::resource::{
  AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceKind, Skill, VersionNumber,
};
use orchester_workspace::{
  agents::team_hub::{Form, TeamHub},
  tools::form::CommandText,
};

use crate::support::{Fixture, http_tool, program, remote};

fn rule() -> Instruction {
  Instruction::new(
    PromptText::new("Be careful.").expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("instruction")
}

fn skill() -> Skill {
  Skill::new(
    "Review a diff.".into(),
    "# Review".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("skill")
}

fn hub(fixture: &Fixture) -> TeamHub {
  fixture
    .workspace
    .team_hub(&fixture.root, &fixture.vault)
    .expect("the hub")
}

#[test]
fn readiness_hub_names_what_each_server_and_tool_is() {
  let fixture = Fixture::team();
  let program_server = fixture.team_server("knowledge", program(&[]));
  let remote_server = fixture.team_server("issues", remote("TOKEN"));
  let command = CommandText {
    program: "cargo".into(),
    ..CommandText::new_tool()
  }
  .tool(None)
  .expect("command");
  let command_tool = fixture.team_resource("checks", command);
  let token = fixture.project_secret("DEPLOY_TOKEN", Some("x"));
  let http = fixture.team_resource("deploy", http_tool(token));
  let rules = fixture.team_resource("rules", rule());

  let read = hub(&fixture);
  assert_eq!(read.form(program_server), Some(Form::McpProgram));
  assert_eq!(read.form(remote_server), Some(Form::McpRemote));
  assert_eq!(read.form(command_tool), Some(Form::ToolCommand));
  assert_eq!(read.form(http), Some(Form::ToolHttp));
  assert_eq!(read.form(rules), None, "an instruction is neither a server nor a tool");
}

#[test]
fn readiness_hub_lists_instructions_then_skills_each_by_name_with_the_number_the_server_gave() {
  let fixture = Fixture::team();
  let beta = fixture.team_resource("beta", rule());
  let alpha = fixture.team_resource("alpha", rule());
  let zed = fixture.team_resource("zed", skill());
  let able = fixture.team_resource("able", skill());
  // Published here and not numbered by the server yet.
  let waiting = fixture.own("waiting", &rule());

  let read = hub(&fixture);
  let order: Vec<_> = read.plain().iter().map(|found| (found.kind, found.id)).collect();
  assert_eq!(
    order,
    [
      (ResourceKind::Instruction, alpha),
      (ResourceKind::Instruction, beta),
      (ResourceKind::Instruction, waiting),
      (ResourceKind::Skill, able),
      (ResourceKind::Skill, zed),
    ]
  );
  let numbers: Vec<_> = read.plain().iter().map(|found| found.number).collect();
  assert_eq!(
    numbers,
    [
      Some(VersionNumber::FIRST),
      Some(VersionNumber::FIRST),
      None,
      Some(VersionNumber::FIRST),
      Some(VersionNumber::FIRST)
    ]
  );
}

#[test]
fn readiness_hub_of_a_local_project_lists_no_instruction_or_skill_of_a_team() {
  let fixture = Fixture::local();
  fixture.own("rules", &rule());
  fixture.own("review", &skill());
  assert!(hub(&fixture).plain().is_empty());
}
