//! The Agents hub of a local project (`Workspace::local_hub`, ORC-842): its profiles and tools as readiness says, each
//! local MCP server on its own (its program, its secret values, and whether it is on), the instructions and skills,
//! what a profile says of itself, and what was archived.

use std::collections::BTreeSet;

use orchester_core::{
  connection::AgentProvider,
  resource::{
    AgentTargets, InjectionEvent, Instruction, InstructionScope, PromptText, ResourceKind, Skill, VersionNumber,
  },
  sharing::{LocalProgramProblem, Missing},
};
use orchester_workspace::{
  agents::local_hub::LocalHub,
  local_archive::Archivable,
  readiness::{Group, ReadinessStanding},
};

use crate::support::{Fixture, at, pipeline, profile_on};

const CLAUDE: AgentProvider = AgentProvider::ClaudeCodeCli;

fn hub(fixture: &Fixture) -> LocalHub {
  fixture
    .workspace
    .local_hub(&fixture.root, &fixture.vault)
    .expect("the hub")
}

#[test]
fn readiness_local_hub_stands_each_server_on_its_own_program_and_switch() {
  let fixture = Fixture::local();
  let file = fixture.program_outside("figma.sh");
  let ready = fixture.local_server("figma", file.to_str().expect("utf-8"));
  let missing = fixture.dir_path().join("missing.sh");
  let needs_path = fixture.local_server("pencil", missing.to_str().expect("utf-8"));
  let off = fixture.local_server("playwright", file.to_str().expect("utf-8"));
  fixture.disable_server(off);

  let read = hub(&fixture);
  assert_eq!(read.mcp_servers().len(), 3, "a local server is listed on its own");
  let figma = read.mcp_server(ready).expect("listed");
  assert_eq!(figma.group(), Group::Ready);
  assert_eq!(figma.standing(), &ReadinessStanding::Ready);
  let pencil = read.mcp_server(needs_path).expect("listed");
  assert_eq!(pencil.group(), Group::NeedsYouHere);
  assert!(matches!(
    pencil.standing().first(),
    Some(Missing::LocalMcpProgram {
      problem: LocalProgramProblem::NotFound,
      ..
    })
  ));
  assert_eq!(read.mcp_server(off).expect("listed").group(), Group::OffHere);
}

#[test]
fn readiness_local_hub_names_the_profiles_that_choose_a_server_and_what_a_profile_runs_on() {
  let fixture = Fixture::local();
  let own = fixture.connection(CLAUDE, "work");
  let file = fixture.program_outside("figma.sh");
  let figma = fixture.local_server("figma", file.to_str().expect("utf-8"));
  let builder = fixture.own("builder", &profile_on(own, &[figma]));
  let reviewer = fixture.own("reviewer", &profile_on(own, &[]));
  fixture.own("delivery", &pipeline(&[Some(builder)], None));

  let read = hub(&fixture);
  assert_eq!(read.profiles_choosing(figma), ["builder"]);
  let sheet = read.profile(builder).expect("a sheet");
  assert_eq!(sheet.cli, Some(CLAUDE));
  assert_eq!(sheet.connection.as_deref(), Some("work"));
  assert_eq!(sheet.mcp_servers.len(), 1);
  assert_eq!(sheet.pipelines, ["delivery"]);
  assert!(read.profile(reviewer).expect("a sheet").pipelines.is_empty());
}

#[test]
fn readiness_local_hub_lists_instructions_and_skills_and_what_was_archived() {
  let fixture = Fixture::local();
  let rules = fixture.own(
    "rules",
    &Instruction::new(
      PromptText::new("Be careful.").expect("prompt"),
      AgentTargets::All,
      BTreeSet::from([InjectionEvent::Start]),
      InstructionScope::Project,
    )
    .expect("instruction"),
  );
  fixture.own(
    "style",
    &Skill::new("Style.".into(), "# Style".into(), Vec::new(), InstructionScope::Project).expect("skill"),
  );
  let file = fixture.program_outside("figma.sh");
  let figma = fixture.local_server("figma", file.to_str().expect("utf-8"));

  let read = hub(&fixture);
  assert_eq!(
    read
      .plain()
      .iter()
      .map(|found| (found.kind, found.number))
      .collect::<Vec<_>>(),
    [
      (ResourceKind::Instruction, VersionNumber::FIRST),
      (ResourceKind::Skill, VersionNumber::FIRST)
    ]
  );
  assert!(read.archived().is_empty());

  fixture.db().archive_local_resource(rules, at(30)).expect("archived");
  fixture.db().archive_mcp_server(figma, at(31)).expect("archived");
  let read = hub(&fixture);
  assert_eq!(read.plain().len(), 1, "an archived instruction leaves the list");
  assert!(read.mcp_server(figma).is_none(), "an archived server leaves the list");
  let archived: Vec<_> = read
    .archived()
    .iter()
    .map(|found| (found.what, found.kind, found.name.as_str()))
    .collect();
  assert!(read.archived().iter().all(|found| found.stopped.is_empty()));
  assert_eq!(
    archived,
    [
      (Archivable::McpServer(figma), ResourceKind::McpServer, "figma"),
      (Archivable::Resource(rules), ResourceKind::Instruction, "rules"),
    ],
    "newest first"
  );
}

#[test]
fn readiness_local_hub_a_server_needs_the_value_of_each_secret_its_variables_take() {
  use std::collections::BTreeMap;

  use orchester_core::{
    connection::mcp::{McpEnvValue, McpEnvironment, McpServer, McpServerName, McpTransport, StdioServer},
    id::ConnectionId,
    resource::{CommandProgram, VariableName},
    store::{McpServerStore, SecretStore},
  };

  let fixture = Fixture::local();
  let file = fixture.program_outside("github.sh");
  let token = fixture.project_secret("GITHUB_TOKEN", None);
  let server = McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("github").expect("name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new(file.to_str().expect("utf-8")).expect("program"),
      Vec::new(),
      None,
    )),
    McpEnvironment::new(BTreeMap::from([(
      VariableName::new("GITHUB_TOKEN").expect("variable"),
      McpEnvValue::Secret(token),
    )]))
    .expect("variables"),
    at(5),
  )
  .expect("server");
  fixture.db().create_mcp_server(&server).expect("stored");

  let read = hub(&fixture);
  let found = read.mcp_server(server.id()).expect("listed");
  assert_eq!(found.group(), Group::NeedsYouHere);
  assert!(matches!(
    found.standing().first(),
    Some(Missing::LocalSecret { secret, .. }) if *secret == token
  ));

  let value = orchester_core::secret::SecretValue::new("x".to_owned()).expect("value");
  let key = fixture
    .db()
    .secret(token)
    .expect("read")
    .expect("kept")
    .vault_key(fixture.db().project().id());
  orchester_core::secret::SecretVault::store(&fixture.vault, &key, &value).expect("in the keychain");
  assert_eq!(
    hub(&fixture).mcp_server(server.id()).expect("listed").standing(),
    &ReadinessStanding::Ready,
    "read again, the value that was set counts"
  );
}
