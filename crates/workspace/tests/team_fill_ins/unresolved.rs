//! A team profile with no connection to run on here: none of its CLI kind, or several and none picked. What its steps
//! run on then names no connection of this computer, which the Agents page and preflight say as "pick one", and adding
//! a connection reads it again: it is read from the team's configuration in memory and this computer's picks.

use orchester_core::{
  connection::{AgentProvider, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
  id::{ConnectionId, ResourceId},
  name::Name,
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_workspace::{
  connections::data::{ConnectionDraft, save},
  pipelines::data::Assignables,
};

use crate::support::{Team, pick};

fn team_profile(team: &Team, cli: AgentProvider) -> ResourceId {
  team.cached_profile("team-builder", &crate::support::team_profile(cli))
}

/// The name of the connection `profile` runs on here, as a step that runs it reads it; `None` while it has none.
pub(crate) fn runs_on(team: &Team, profile: ResourceId) -> Option<String> {
  let config = team
    .workspace
    .team_read(&team.root)
    .config()
    .cloned()
    .expect("the team's configuration");
  let assignables = Assignables::of_team(&team.db(), &config).expect("read");
  let choice = assignables.profile(profile).expect("the team's profile");
  choice.connection.as_deref().map(str::to_owned)
}

/// The name of connection `id` here.
pub(crate) fn named(team: &Team, id: ConnectionId) -> Option<String> {
  let connection = team.db().connection(id).expect("read").expect("the connection");
  Some(connection.name().as_str().to_owned())
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
    None,
    "no connection of the profile's kind: none to run on"
  );
}

#[test]
fn team_fill_ins_several_connections_of_the_kind_without_a_pick_leave_the_profile_unresolved() {
  let team = Team::new();
  let profile = team_profile(&team, AgentProvider::CodexCli);

  add_connection(&team, AgentProvider::ClaudeCodeCli, "claude two");
  assert_eq!(
    runs_on(&team, profile),
    named(&team, team.codex),
    "the only Codex connection runs it"
  );

  add_connection(&team, AgentProvider::CodexCli, "codex two");
  assert_eq!(runs_on(&team, profile), None, "two Codex connections and no pick");

  team
    .workspace
    .pick_team_connection(&team.root, profile, pick(team.codex))
    .expect("a pick");
  assert_eq!(runs_on(&team, profile), named(&team, team.codex));
  add_connection(&team, AgentProvider::CodexCli, "codex three");
  assert_eq!(
    runs_on(&team, profile),
    named(&team, team.codex),
    "a pick stays when more are added"
  );
}
