//! The program path of a team MCP server.

use std::path::Path;

use orchester_core::run::Answerer;
use orchester_workspace::team_fill_ins::FillInError;

use crate::support::Team;

#[test]
fn team_fill_ins_a_person_sets_the_program_of_a_team_mcp_server_and_clears_it() {
  let team = Team::new();
  let program = team.program_file();
  let before = team.connection_changes();
  team
    .workspace
    .set_team_mcp_program(&team.root, team.program_server, &program, Answerer::Person)
    .expect("a person chose the file");
  assert_eq!(team.program_set(team.program_server), Some(program));
  assert_eq!(team.connection_changes(), before + 1);

  team
    .workspace
    .clear_team_mcp_program(&team.root, team.program_server)
    .expect("cleared");
  assert_eq!(team.program_set(team.program_server), None);
  team
    .workspace
    .clear_team_mcp_program(&team.root, team.program_server)
    .expect("clearing again is not an error");
  assert_eq!(team.connection_changes(), before + 3);
}

#[test]
fn team_fill_ins_automation_cannot_set_a_program_but_can_clear_it() {
  let team = Team::new();
  let program = team.program_file();
  let refused = team
    .workspace
    .set_team_mcp_program(&team.root, team.program_server, &program, Answerer::Automation);
  assert!(matches!(refused, Err(FillInError::PersonOnly)), "{refused:?}");
  assert_eq!(team.program_set(team.program_server), None, "nothing changed");

  team
    .workspace
    .set_team_mcp_program(&team.root, team.program_server, &program, Answerer::Person)
    .expect("a person chose the file");
  team
    .workspace
    .clear_team_mcp_program(&team.root, team.program_server)
    .expect("clearing narrows, so anyone may");
  assert_eq!(team.program_set(team.program_server), None);
}

#[test]
fn team_fill_ins_a_program_is_an_absolute_path_to_a_file_of_a_local_team_server() {
  let team = Team::new();
  let set = |server, path: &Path| {
    team
      .workspace
      .set_team_mcp_program(&team.root, server, path, Answerer::Person)
  };
  let relative = set(team.program_server, Path::new("bin/mcp-server"));
  assert!(matches!(relative, Err(FillInError::ProgramNotAbsolute)), "{relative:?}");
  let missing = set(team.program_server, &team.root.join("nowhere"));
  assert!(matches!(missing, Err(FillInError::ProgramNotAFile)), "{missing:?}");
  let folder = set(team.program_server, &team.root);
  assert!(matches!(folder, Err(FillInError::ProgramNotAFile)), "{folder:?}");

  let program = team.program_file();
  let remote = set(team.remote_server, &program);
  assert!(
    matches!(remote, Err(FillInError::RemoteServer(id)) if id == team.remote_server),
    "{remote:?}"
  );
  let profile = set(team.claude_profile, &program);
  assert!(
    matches!(profile, Err(FillInError::NotTeamResource { .. })),
    "{profile:?}"
  );
  assert_eq!(team.program_set(team.program_server), None);
}

#[test]
fn team_fill_ins_a_program_path_refreshes_every_project_of_its_team_server() {
  let team = Team::new();
  let program = team.program_file();
  team.workspace.open(&team.root).expect("opened");
  let sibling = team.add_project("sibling", "SIB", team.server);
  let elsewhere = team.add_project("elsewhere", "ELS", orchester_core::id::ServerId::generate());
  let sets = [&team.root, &sibling, &elsewhere].map(|root| team.changes_of(root));

  team
    .workspace
    .set_team_mcp_program(&team.root, team.program_server, &program, Answerer::Person)
    .expect("a person chose the file");
  assert_eq!(team.changes_of(&team.root), sets[0] + 1, "the project that set it");
  assert_eq!(team.changes_of(&sibling), sets[1] + 1, "a project of the same server");
  assert_eq!(team.changes_of(&elsewhere), sets[2], "a project of another server");

  team
    .workspace
    .clear_team_mcp_program(&team.root, team.program_server)
    .expect("cleared");
  assert_eq!(team.changes_of(&team.root), sets[0] + 2);
  assert_eq!(team.changes_of(&sibling), sets[1] + 2, "clearing reaches it too");
  assert_eq!(team.changes_of(&elsewhere), sets[2]);
}
