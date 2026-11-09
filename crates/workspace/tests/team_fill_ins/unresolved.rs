//! A team profile with no connection to run on here: none of its CLI kind, or several and none picked. Its local body
//! then names no connection of this computer, which the Agents page, publishing and preflight say as "pick one", and
//! adding a connection reads it again.

use orchester_core::{
  connection::{AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
  id::{ConnectionId, ResourceId},
  name::Name,
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, PublishError, TeamAgentProfile, VersionNumber},
  time::Timestamp,
};
use orchester_store::StoreError;
use orchester_workspace::{
  agents::data::{ProfileChange, apply_in},
  connections::data::{ConnectionDraft, save},
};

use crate::support::{Team, cached, pick, team_resource};

fn team_profile(team: &Team, cli: AgentProvider) -> ResourceId {
  let body = TeamAgentProfile::new(
    Name::new("Builder").expect("role"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    (
      cli,
      ModelId::new("gpt-5.5").expect("model"),
      ProviderSettings::default(),
    ),
  );
  let mut db = team.db();
  let id = team_resource(
    &mut db,
    "team-builder",
    AgentProfile::from_team(&body, ConnectionId::generate()),
    Vec::new(),
  );
  let mut version = cached(AgentProfile::from_team(&body, ConnectionId::generate()), Vec::new());
  version.server_body = serde_json::to_value(&body).expect("a team profile serializes");
  db.cache_shared_version(id, &version).expect("the team form cached");
  id
}

fn runs_on(team: &Team, profile: ResourceId) -> ConnectionId {
  team
    .db()
    .cached_version::<AgentProfile>(profile, VersionNumber::FIRST)
    .expect("read")
    .expect("cached")
    .local_body
    .connection()
}

fn add_connection(team: &Team, provider: AgentProvider, label: &str) -> ConnectionId {
  let draft = ConnectionDraft {
    id: None,
    name: Name::new(label).expect("name"),
    provider,
    executable: team.root.join(format!("{label}.exe")),
    check: ConnectionCheck {
      at: Timestamp::from_unix_millis(1),
      result: CheckResult::Available {
        version: "2.1.89".into(),
        sign_in: SignIn::SignedOut,
        catalog: ModelCatalog::new(CatalogSource::DocumentedAliases, Vec::new()),
      },
    },
  };
  save(&mut team.db(), draft, Timestamp::from_unix_millis(2)).expect("saved")
}

#[test]
fn team_fill_ins_no_connection_of_the_profiles_kind_leaves_it_unresolved() {
  let team = Team::new();
  let profile = team_profile(&team, AgentProvider::AnthropicApi);
  add_connection(&team, AgentProvider::ClaudeCodeCli, "claude two");
  assert_eq!(
    runs_on(&team, profile),
    AgentProfile::unresolved_connection(profile),
    "no connection of the profile's kind: none to run on"
  );
}

#[test]
fn team_fill_ins_several_connections_of_the_kind_without_a_pick_leave_the_profile_unresolved() {
  let team = Team::new();
  let profile = team_profile(&team, AgentProvider::CodexCli);

  add_connection(&team, AgentProvider::ClaudeCodeCli, "claude two");
  assert_eq!(runs_on(&team, profile), team.codex, "the only Codex connection runs it");

  add_connection(&team, AgentProvider::CodexCli, "codex two");
  assert_eq!(
    runs_on(&team, profile),
    AgentProfile::unresolved_connection(profile),
    "two Codex connections and no pick"
  );

  team
    .workspace
    .pick_team_connection(&team.root, profile, pick(team.codex))
    .expect("a pick");
  assert_eq!(runs_on(&team, profile), team.codex);
  add_connection(&team, AgentProvider::CodexCli, "codex three");
  assert_eq!(runs_on(&team, profile), team.codex, "a pick stays when more are added");
}

#[test]
fn team_fill_ins_a_profile_with_no_connection_picked_cannot_be_published() {
  let team = Team::new();
  let profile = team_profile(&team, AgentProvider::CodexCli);
  add_connection(&team, AgentProvider::CodexCli, "codex two");
  let body = team
    .db()
    .cached_version::<AgentProfile>(profile, VersionNumber::FIRST)
    .expect("read")
    .expect("cached")
    .local_body;
  assert!(body.awaits_connection_pick(profile));

  apply_in(&team.root, ProfileChange::SaveDraft(profile, body)).expect("a draft");
  assert!(
    matches!(
      apply_in(&team.root, ProfileChange::Publish(profile)),
      Err(StoreError::Publish(PublishError::ConnectionNotPicked))
    ),
    "the profile says a connection must be picked, not that it is gone"
  );
}
