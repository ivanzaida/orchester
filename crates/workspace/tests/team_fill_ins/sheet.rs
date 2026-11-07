//! What the panel of a team profile shows (ORC-832): what the team holds of the profile and the connections of its CLI
//! kind on this computer, which the pick and the only connection of the kind decide.

use orchester_core::{
  connection::AgentProvider,
  id::{ConnectionId, ResourceId},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, Slug, TeamAgentProfile},
  sharing::TaskRevision,
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ResourceSharing;

use crate::support::{MemoryVault, Team, cached, connection, name, pick};

/// A team profile in the team's form, running on Claude Code and using no MCP server.
fn builder(team: &Team) -> ResourceId {
  let body = TeamAgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    (
      AgentProvider::ClaudeCodeCli,
      ModelId::new("claude-opus-5-5").expect("model"),
      ProviderSettings::default(),
    ),
  );
  let local = AgentProfile::from_team(&body, ConnectionId::generate());
  let mut db = team.db();
  let id = db
    .create_resource(
      Slug::new("team-builder").expect("slug"),
      name("Builder"),
      &local,
      Timestamp::from_unix_millis(10),
    )
    .expect("resource")
    .id();
  db.set_resource_sharing(
    id,
    Some(ResourceSharing::Shared(TaskRevision::try_from(1).expect("revision"))),
  )
  .expect("shared");
  let mut version = cached(local, Vec::new());
  version.server_body = serde_json::to_value(&body).expect("json");
  db.cache_shared_version(id, &version).expect("cached");
  id
}

#[test]
fn team_fill_ins_a_profile_sheet_lists_the_connections_of_its_cli_kind_and_runs_on_the_only_one() {
  let team = Team::new();
  let profile = builder(&team);
  let hub = team
    .workspace
    .team_hub(&team.root, &MemoryVault::default())
    .expect("hub");
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
  assert!(
    hub.profile(team.legacy_profile).is_none(),
    "a version cached before the model has no sheet"
  );
  assert!(hub.profile(team.remote_server).is_none(), "only profiles have one");
}

#[test]
fn team_fill_ins_a_profile_sheet_runs_on_nothing_until_the_person_picks_among_several() {
  let team = Team::new();
  let profile = builder(&team);
  let second = connection(&mut team.db(), &team.root, AgentProvider::ClaudeCodeCli, "Home");
  let vault = MemoryVault::default();
  let sheet = |team: &Team| {
    team
      .workspace
      .team_hub(&team.root, &vault)
      .expect("hub")
      .profile(profile)
      .cloned()
      .expect("a sheet")
  };
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
