//! What the panel of a team profile shows (ORC-832): what the team holds of the profile and the connections of its CLI
//! kind on this computer, which the pick and the only connection of the kind decide.

use orchester_core::{
  connection::AgentProvider,
  id::ResourceId,
  resource::{ModelId, PromptText, ProviderSettings, ResourceKind, TeamAgentProfile},
  run::Answerer,
};
use orchester_workspace::agents::team_mcp::McpRuns;

use crate::support::{MemoryVault, Team, connection, hub, name, pick};

/// A team profile in the team's form, running on Claude Code and using no MCP server.
fn builder(team: &Team) -> ResourceId {
  builder_using(team, Vec::new())
}

/// [`builder`] using the MCP servers `servers`.
fn builder_using(team: &Team, servers: Vec<ResourceId>) -> ResourceId {
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
  .with_mcp_servers(servers)
  .expect("servers");
  team.keep(
    "team-builder",
    ResourceKind::AgentProfile,
    serde_json::to_value(&body).expect("json"),
  )
}

#[test]
fn team_fill_ins_a_profile_sheet_lists_the_connections_of_its_cli_kind_and_runs_on_the_only_one() {
  let team = Team::new();
  let profile = builder(&team);
  let hub = hub(&team, &MemoryVault::default());
  let sheet = hub.profile(profile).expect("a team profile in the team's form");
  assert_eq!(sheet.cli, AgentProvider::ClaudeCodeCli);
  assert_eq!(sheet.model, "claude-opus-5-5");
  assert_eq!(
    sheet.connections.iter().map(|choice| choice.id).collect::<Vec<_>>(),
    vec![team.claude],
    "the Codex connection is another kind"
  );
  assert_eq!(sheet.runs_on().map(|choice| choice.id), Some(team.claude));
  assert_eq!(sheet.picked, None);
  assert!(hub.profile(team.remote_server).is_none(), "only profiles have one");
}

#[test]
fn team_fill_ins_a_profile_sheet_runs_on_nothing_until_the_person_picks_among_several() {
  let team = Team::new();
  let profile = builder(&team);
  let second = connection(&mut team.db(), &team.root, AgentProvider::ClaudeCodeCli, "Home");
  let vault = MemoryVault::default();
  let sheet = |team: &Team| hub(team, &vault).profile(profile).cloned().expect("a sheet");
  let before = sheet(&team);
  assert_eq!(before.connections.len(), 2);
  assert_eq!(before.runs_on(), None, "several, and none picked");

  team
    .workspace
    .pick_team_connection(&team.root, profile, pick(second))
    .expect("a pick");
  let after = sheet(&team);
  assert_eq!(after.picked, Some(second));
  assert_eq!(after.runs_on().map(|choice| choice.id), Some(second));
}

#[test]
fn team_fill_ins_an_mcp_server_sheet_says_how_it_runs_who_uses_it_and_what_this_computer_set() {
  let team = Team::new();
  builder_using(&team, vec![team.program_server]);
  let vault = MemoryVault::default();
  let sheet = |team: &Team, server| hub(team, &vault).mcp_server(server).cloned().expect("a sheet");
  let program = sheet(&team, team.program_server);
  assert_eq!(
    program.runs,
    McpRuns::Program {
      program: "npx".to_owned(),
      args: Vec::new(),
      folder: None,
      variables: Vec::new()
    }
  );
  assert_eq!(
    program.used_by,
    vec!["Builder".to_owned()],
    "the profile that chooses it"
  );
  assert_eq!(program.program, None, "no path until the person sets one");
  assert_eq!(program.switch, None, "nobody decided");
  assert!(program.published.is_some(), "the version the server confirmed");

  let file = team.program_file();
  team
    .workspace
    .set_team_mcp_program(&team.root, team.program_server, &file, Answerer::Person)
    .expect("a path");
  team
    .workspace
    .switch_team_mcp_on_here(&team.root, team.program_server, true, Answerer::Person)
    .expect("on");
  let set = sheet(&team, team.program_server);
  assert_eq!(set.program.map(|set| set.path), Some(file));
  assert_eq!(set.switch, Some(true));

  let remote = sheet(&team, team.remote_server);
  assert!(matches!(remote.runs, McpRuns::Remote { ref url, .. } if url == "https://mcp.example.org/x"));
  assert!(remote.used_by.is_empty());
  assert_eq!(remote.program, None, "a remote server has no program");
}
