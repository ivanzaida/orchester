//! Which team MCP servers are on here.

use orchester_core::{id::ResourceId, run::Answerer};
use orchester_workspace::team_fill_ins::FillInError;

use crate::support::Team;

#[test]
fn team_fill_ins_a_person_turns_a_team_mcp_server_on_and_anyone_turns_it_off() {
  let team = Team::new();
  let server = team.program_server;
  let on = || team.db().team_mcp_on_here(server).expect("read");
  assert!(!on(), "off until a person turns it on");

  let refused = team
    .workspace
    .switch_team_mcp_on_here(&team.root, server, true, Answerer::Automation);
  assert!(matches!(refused, Err(FillInError::PersonOnly)), "{refused:?}");
  assert!(!on());

  let before = team.connection_changes();
  team
    .workspace
    .switch_team_mcp_on_here(&team.root, server, true, Answerer::Person)
    .expect("on");
  assert!(on());
  assert_eq!(team.connection_changes(), before + 1);

  team
    .workspace
    .switch_team_mcp_on_here(&team.root, server, false, Answerer::Automation)
    .expect("off narrows, so anyone may");
  assert!(!on());
}

#[test]
fn team_fill_ins_only_a_team_mcp_server_is_turned_on() {
  let team = Team::new();
  for not_a_server in [team.claude_profile, team.local_profile, ResourceId::generate()] {
    let refused = team
      .workspace
      .switch_team_mcp_on_here(&team.root, not_a_server, true, Answerer::Person);
    assert!(
      matches!(refused, Err(FillInError::NotTeamResource { .. })),
      "{refused:?}"
    );
    assert!(!team.db().team_mcp_on_here(not_a_server).expect("read"));
  }
}
