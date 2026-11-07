//! The connection (and account) a team profile runs on.

use orchester_core::{account::AccountCli, id::ConnectionId};
use orchester_store::ConnectionPick;
use orchester_workspace::team_fill_ins::FillInError;

use crate::support::{Team, pick};

#[test]
fn team_fill_ins_pick_a_connection_of_the_profiles_cli_kind() {
  let team = Team::new();
  let before = team.connection_changes();
  team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, pick(team.claude))
    .expect("a Claude Code connection for a Claude Code profile");
  assert_eq!(
    team.db().connection_pick(team.claude_profile).expect("read"),
    Some(pick(team.claude))
  );
  assert_eq!(team.connection_changes(), before + 1, "screens read again");

  let refused = team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, pick(team.codex));
  assert!(
    matches!(refused, Err(FillInError::ConnectionDoesNotFit { .. })),
    "{refused:?}"
  );
  assert_eq!(
    team.db().connection_pick(team.claude_profile).expect("read"),
    Some(pick(team.claude)),
    "the earlier pick stays"
  );
  assert_eq!(team.connection_changes(), before + 1, "a refusal changes nothing");
}

#[test]
fn team_fill_ins_a_version_cached_before_the_cli_kind_still_fits_by_its_connections_role() {
  let team = Team::new();
  team
    .workspace
    .pick_team_connection(&team.root, team.legacy_profile, pick(team.claude))
    .expect("a Claude Code connection for a profile that named one");
  let refused = team
    .workspace
    .pick_team_connection(&team.root, team.legacy_profile, pick(team.codex));
  assert!(
    matches!(refused, Err(FillInError::ConnectionDoesNotFit { .. })),
    "{refused:?}"
  );
}

#[test]
fn team_fill_ins_a_profile_of_no_known_kind_takes_any_agent_connection() {
  let team = Team::new();
  for connection in [team.claude, team.codex] {
    team
      .workspace
      .pick_team_connection(&team.root, team.open_profile, pick(connection))
      .expect("an agent connection");
  }
  assert_eq!(
    team.db().connection_pick(team.open_profile).expect("read"),
    Some(pick(team.codex))
  );
}

#[test]
fn team_fill_ins_a_pick_names_a_team_profile_and_an_existing_connection() {
  let team = Team::new();
  let local = team
    .workspace
    .pick_team_connection(&team.root, team.local_profile, pick(team.claude));
  assert!(matches!(local, Err(FillInError::NotTeamResource { .. })), "{local:?}");
  let wrong_kind = team
    .workspace
    .pick_team_connection(&team.root, team.program_server, pick(team.claude));
  assert!(
    matches!(wrong_kind, Err(FillInError::NotTeamResource { .. })),
    "{wrong_kind:?}"
  );
  let missing = ConnectionId::generate();
  let gone = team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, pick(missing));
  assert!(
    matches!(gone, Err(FillInError::ConnectionNotFound(id)) if id == missing),
    "{gone:?}"
  );
  assert_eq!(team.db().connection_pick(team.local_profile).expect("read"), None);
}

#[test]
fn team_fill_ins_an_account_is_a_listed_account_of_the_connections_cli() {
  let team = Team::new();
  let with = |account| ConnectionPick {
    connection: team.claude,
    account: Some(account),
  };
  let default = AccountCli::ClaudeCode.default_account();
  team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, with(default))
    .expect("the Default account of Claude Code");
  assert_eq!(
    team.db().connection_pick(team.claude_profile).expect("read"),
    Some(with(default))
  );

  let other = AccountCli::Codex.default_account();
  let refused = team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, with(other));
  assert!(
    matches!(refused, Err(FillInError::AccountOfOtherCli(id)) if id == other),
    "{refused:?}"
  );
  let unknown = orchester_core::id::AgentAccountId::generate();
  let gone = team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, with(unknown));
  assert!(
    matches!(gone, Err(FillInError::AccountNotFound(id)) if id == unknown),
    "{gone:?}"
  );
}
