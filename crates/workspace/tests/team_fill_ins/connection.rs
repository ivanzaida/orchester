//! The connection (and account) a team profile runs on.

use orchester_core::{
  account::AccountCli,
  connection::AgentProvider,
  id::ConnectionId,
  name::Name,
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, TeamAgentProfile, VersionNumber},
};
use orchester_store::ConnectionPick;
use orchester_workspace::team_fill_ins::FillInError;

use crate::support::{Team, cached, pick, team_resource};

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

/// A profile in the team's form runs on the connection of its CLI kind a person picked: the local body, which runs and
/// the Agents page read, follows the pick; the server's body is never changed.
#[test]
fn team_fill_ins_a_pick_points_a_team_profile_at_the_connection_it_runs_on() {
  let team = Team::new();
  let second = {
    let mut db = team.db();
    crate::support::connection(&mut db, &team.root, AgentProvider::CodexCli, "codex work")
  };
  let body = TeamAgentProfile::new(
    Name::new("Builder").expect("role"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    (
      AgentProvider::CodexCli,
      ModelId::new("gpt-5.5").expect("model"),
      ProviderSettings::default(),
    ),
  );
  let server_body = serde_json::to_value(&body).expect("a team profile serializes");
  let profile = {
    let mut db = team.db();
    let id = team_resource(
      &mut db,
      "team-builder",
      AgentProfile::from_team(&body, ConnectionId::generate()),
      Vec::new(),
    );
    let mut version = cached(AgentProfile::from_team(&body, ConnectionId::generate()), Vec::new());
    version.server_body = server_body.clone();
    db.cache_shared_version(id, &version).expect("the team form cached");
    id
  };
  let runs_on = |team: &Team| {
    let cached = team
      .db()
      .cached_version::<AgentProfile>(profile, VersionNumber::FIRST)
      .expect("read")
      .expect("cached");
    (cached.local_body.connection(), cached.server_body)
  };

  team
    .workspace
    .pick_team_connection(&team.root, profile, pick(second))
    .expect("a Codex connection for a Codex profile");

  assert_eq!(runs_on(&team), (second, server_body.clone()));
  team
    .workspace
    .pick_team_connection(&team.root, profile, pick(team.codex))
    .expect("another pick replaces it");
  assert_eq!(runs_on(&team), (team.codex, server_body));
}
