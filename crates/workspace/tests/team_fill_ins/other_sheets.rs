//! What the panels of a team tool, instruction and skill show (ORC-834): what the team holds of them, which profiles
//! use them and who published the version this computer uses. A tool names its secret by id, never a value.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  connection::AgentProvider,
  id::{ResourceId, SecretId},
  resource::{
    AccessPolicy, AccessWindow, AgentTargets, ApprovalMode, CommandAction, CommandArg, CommandProgram, DatabaseQuery,
    DeniedBehavior, FieldName, HttpAuth, HttpMethod, HttpRequest, InjectionEvent, Instruction, InstructionScope,
    ModelId, PromptText, ProviderSettings, RequestTarget, ResourceKind, Skill, SkillFile, SkillFilePath,
    TeamAgentProfile, Tool, ToolAction,
  },
};
use orchester_workspace::agents::{
  team_text::{Added, Reaches, SKILL_DOCUMENT},
  team_tool::{ArgumentPlace, ToolRuns, ToolSecret},
};

use crate::support::{MemoryVault, Team, hub, name};

fn policy(approval: ApprovalMode) -> AccessPolicy {
  AccessPolicy::new(
    BTreeMap::new(),
    name("Production"),
    approval,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::FailStep,
  )
  .expect("policy")
}

/// A team profile Builder that uses `instruction`, `skill` and `tool`.
fn builder(team: &Team, (instruction, skill, tool): (ResourceId, ResourceId, ResourceId)) {
  let body = TeamAgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    (
      AgentProvider::ClaudeCodeCli,
      ModelId::new("claude-opus-5-5").expect("model"),
      ProviderSettings::default(),
    ),
  )
  .with_instruction_resources(vec![instruction])
  .expect("instructions")
  .with_skills(vec![skill])
  .expect("skills")
  .with_tools(vec![tool])
  .expect("tools");
  team.keep(
    "team-builder",
    ResourceKind::AgentProfile,
    serde_json::to_value(&body).expect("json"),
  );
}

/// An HTTP tool with one argument that holds the project secret `secret` by id.
fn http_tool(secret: SecretId) -> Tool {
  let request = HttpRequest::new(
    HttpMethod::Get,
    "https://sentry.io/api/0/issues/".into(),
    "sentry.io".into(),
    HttpAuth::Bearer { secret },
    BTreeMap::from([(
      FieldName::new("query").expect("field"),
      RequestTarget::Query("query".into()),
    )]),
    BTreeMap::new(),
  )
  .expect("request");
  Tool::new("Issues", ToolAction::Http(request), Some(policy(ApprovalMode::Never))).expect("tool")
}

fn command_tool() -> Tool {
  Tool::command(
    "",
    CommandAction::new(
      CommandProgram::new("cargo").expect("program"),
      vec![CommandArg::new("check").expect("argument")],
    ),
  )
}

fn database_tool() -> Tool {
  let query = DatabaseQuery::new(SecretId::generate(), "select 1".into()).expect("query");
  Tool::new(
    "",
    ToolAction::DatabaseQuery(query),
    Some(policy(ApprovalMode::EveryCall)),
  )
  .expect("tool")
}

#[test]
fn team_fill_ins_other_a_tool_sheet_says_what_it_runs_calls_and_names_by_id() {
  let team = Team::new();
  let secret = SecretId::generate();
  let (http_id, command_id, database_id) = (
    team.team_resource("sentry", &http_tool(secret)),
    team.team_resource("cargo", &command_tool()),
    team.team_resource("orders", &database_tool()),
  );
  let hub = hub(&team, &MemoryVault::default());
  let sheet = hub.tool(http_id).expect("a sheet");
  assert_eq!(sheet.description, "Issues");
  assert_eq!(sheet.secret, ToolSecret::ById(secret));
  assert_eq!(sheet.approval, Some(ApprovalMode::Never));
  let ToolRuns::Http { method, url, arguments } = &sheet.runs else {
    panic!("an HTTP tool: {:?}", sheet.runs)
  };
  assert_eq!(
    (*method, url.as_str()),
    (HttpMethod::Get, "https://sentry.io/api/0/issues/")
  );
  assert_eq!(
    arguments.iter().map(|a| (a.name.as_str(), a.place)).collect::<Vec<_>>(),
    vec![("query", ArgumentPlace::Query)]
  );
  assert!(
    sheet
      .published
      .as_ref()
      .is_some_and(|published| published.by == "Olena")
  );
  let sheet = hub.tool(command_id).expect("a sheet");
  assert_eq!(
    (sheet.secret, sheet.approval, &sheet.runs),
    (
      ToolSecret::None,
      None,
      &ToolRuns::Command {
        line: "cargo check".into(),
        timeout_secs: 600
      }
    )
  );
  let sheet = hub.tool(database_id).expect("a sheet");
  assert_eq!(
    sheet.runs,
    ToolRuns::Database {
      query: "select 1".into()
    }
  );
  assert_eq!(sheet.approval, Some(ApprovalMode::EveryCall));
  assert!(hub.tool(ResourceId::generate()).is_none());
}

#[test]
fn team_fill_ins_other_instruction_and_skill_sheets_list_where_they_are_added_and_who_uses_them() {
  let team = Team::new();
  let rules = Instruction::new(
    PromptText::new("Be careful.").expect("prompt"),
    AgentTargets::Roles(BTreeSet::from([name("Builder")])),
    BTreeSet::from([InjectionEvent::Start, InjectionEvent::Resume]),
    InstructionScope::Project,
  )
  .expect("instruction");
  let handoff = Instruction::new(
    PromptText::new("Hand off.").expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Resume]),
    InstructionScope::AgentRole { role: name("Builder") },
  )
  .expect("instruction");
  let skill = Skill::new(
    "Review a diff.".into(),
    "# Review".into(),
    vec![SkillFile {
      path: SkillFilePath::new("references/checklist.md").expect("path"),
      content: orchester_core::blob::ContentHash::new(&"b".repeat(64)).expect("hash"),
      size: 3,
    }],
    InstructionScope::Project,
  )
  .expect("skill");
  let tool = Tool::command(
    "",
    CommandAction::new(CommandProgram::new("cargo").expect("program"), Vec::new()),
  );
  let (rules_id, handoff_id, skill_id, tool_id) = (
    team.team_resource("rules", &rules),
    team.team_resource("handoff", &handoff),
    team.team_resource("review", &skill),
    team.team_resource("cargo", &tool),
  );
  builder(&team, (rules_id, skill_id, tool_id));
  let hub = hub(&team, &MemoryVault::default());
  let sheet = hub.instruction(rules_id).expect("a sheet");
  assert_eq!(sheet.text, "Be careful.");
  assert_eq!(sheet.added, Added::Project);
  assert_eq!(sheet.reaches, Reaches::Roles(vec!["Builder".into()]));
  assert_eq!(sheet.events, vec![InjectionEvent::Start, InjectionEvent::Resume]);
  assert_eq!(sheet.used_by, vec!["team-builder".to_owned()]);
  let sheet = hub.instruction(handoff_id).expect("a sheet");
  assert_eq!(sheet.added, Added::Role("Builder".into()));
  assert!(sheet.used_by.is_empty());
  let sheet = hub.skill(skill_id).expect("a sheet");
  assert_eq!(sheet.description, "Review a diff.");
  assert_eq!(
    sheet.files,
    vec![SKILL_DOCUMENT.to_owned(), "references/checklist.md".to_owned()]
  );
  assert_eq!(sheet.used_by, vec!["team-builder".to_owned()]);
  assert_eq!(
    hub.tool(tool_id).map(|sheet| sheet.used_by.clone()),
    Some(vec!["team-builder".to_owned()])
  );
  assert!(hub.skill(rules_id).is_none() && hub.instruction(skill_id).is_none());
}
