//! The panel of a team profile (`Agents · Profile` `NDZXC` C1, `WPvZx` C2, `QOBF2` C3, ORC-832): what the team holds
//! of it, the connection it runs on here (the only one of its kind, a pick among several, or none to add), what is
//! missing and the pinned fix of the first missing part.
//! `cargo test -p orchester-app --test agents team_hub_profile`.

use std::{collections::BTreeMap, path::Path};

use orchester_app::routes::{AccountsView, HubFilter, HubView, Route};
use orchester_core::{
  connection::{
    AgentConnection, AgentProvider, CatalogModel, CatalogSource, CheckResult, ConnectionCheck, ModelCatalog, SignIn,
  },
  id::{ConnectionId, ResourceId},
  name::Name,
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, TeamAgentProfile, TeamMcpServer},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::json;

use super::{
  harness::Agents,
  team_hub::{cached, open, team},
};

/// A Claude Code connection `name` whose last check found `version`.
fn claude(root: &Path, name: &str, version: &str) -> ConnectionId {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new(name).expect("name"),
    AgentProvider::ClaudeCodeCli,
    root.join(name.replace(' ', "-")),
    Timestamp::now(),
  )
  .expect("absolute program");
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::now(),
      result: CheckResult::Available {
        version: version.into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(
          CatalogSource::Provider,
          vec![CatalogModel::new(
            ModelId::new("claude-opus-5-5").expect("model"),
            vec!["high".into()],
          )],
        ),
      },
    })
    .expect("check");
  let id = connection.id();
  let mut db = ProjectDb::open(root).expect("database");
  db.create_connection(&connection.into_connection()).expect("connection");
  id
}

/// The team's profile Builder on Claude Code, using the MCP server `gh`, which is on here and takes the secret header
/// `GITHUB_TOKEN`; so the only thing it can miss besides a connection is that secret's value. The profile's id and the
/// server's.
fn builder(agents: &Agents) -> (ResourceId, ResourceId) {
  let mut db = ProjectDb::open(&agents.root).expect("database");
  let body = json!({"name": "gh", "launch": {
    "kind": "remote", "url": "https://mcp.example.org/gh",
    "secret_headers": [{"header": "Authorization", "secret": "GITHUB_TOKEN", "prefix": "Bearer "}]
  }});
  let server: TeamMcpServer = serde_json::from_value(body.clone()).expect("a team MCP server");
  let server = cached(&mut db, ("gh", "gh"), body, &server);
  db.set_team_mcp_on_here(server, true).expect("on here");
  let team = TeamAgentProfile::new(
    Name::new("Builder").expect("name"),
    String::new(),
    PromptText::new("Do the work.").expect("prompt"),
    (
      AgentProvider::ClaudeCodeCli,
      ModelId::new("claude-opus-5-5").expect("model"),
      ProviderSettings::new(BTreeMap::from([("reasoning_effort".to_owned(), "high".to_owned())])).expect("settings"),
    ),
  )
  .with_mcp_servers(vec![server])
  .expect("servers");
  let local = AgentProfile::from_team(&team, ConnectionId::generate());
  let profile = cached(
    &mut db,
    ("team-builder", "Builder"),
    serde_json::to_value(&team).expect("json"),
    &local,
  );
  (profile, server)
}

fn show(agents: &mut Agents, profile: ResourceId) {
  open(agents, HubView::new(HubFilter::Profiles, Some(profile)));
  agents.until("the profile panel", |s| s.exists("team-hub-from-team-cli-value"));
}

#[test]
fn team_hub_profile_with_one_connection_of_its_kind_runs_on_it_without_asking() {
  let mut agents = Agents::new(true);
  team(&agents);
  claude(&agents.root, "Claude 2", "2.1.173");
  let (profile, _) = builder(&agents);
  show(&mut agents, profile);
  // What the team holds: the kind, never a connection, the model, the effort, the servers and the secrets by name.
  assert_eq!(agents.text("team-hub-from-team-cli-value"), "Claude Code");
  assert_eq!(agents.text("team-hub-from-team-model-value"), "claude-opus-5-5");
  assert_eq!(agents.text("team-hub-from-team-effort-value"), "High");
  assert_eq!(agents.text("team-hub-from-team-mcp-value-0-label"), "gh");
  assert_eq!(agents.text("team-hub-from-team-secrets-value-0-label"), "GITHUB_TOKEN");
  assert!(
    !agents.exists("team-hub-from-team-tools"),
    "a profile with no tool has no Tools row"
  );
  // On this computer: the only connection of its kind, chosen without asking.
  assert_eq!(agents.text("team-hub-runs-on-name"), "Claude Code · Claude 2");
  assert_eq!(
    agents.text("team-hub-runs-on-only"),
    "· the only Claude Code connection here"
  );
  assert_eq!(agents.text("team-hub-runs-on-sub"), "chosen without asking");
  assert!(!agents.exists("team-hub-pick"));
  // Ready here?: the connection is fine, the secret has no value, and the pinned action adds it.
  assert_eq!(
    agents.text("team-hub-ready-connection-words"),
    "Claude Code · Claude 2 · claude 2.1.173"
  );
  assert_eq!(
    agents.text("team-hub-ready-secret-GITHUB_TOKEN-words"),
    "No value set here"
  );
  assert_eq!(agents.text("team-hub-readiness-count"), "1 missing");
  assert!(
    agents
      .text("team-hub-fix-add-value")
      .contains("Add value for GITHUB_TOKEN")
  );
  assert_eq!(
    agents.text("team-hub-actions-hint"),
    "Builder runs here once nothing is missing"
  );
  assert!(agents.exists("team-hub-ready-secret-GITHUB_TOKEN-add-value"));
  assert!(agents.exists("team-hub-edit"), "an admin has Edit in the head");
}

#[test]
fn team_hub_profile_with_several_connections_of_its_kind_asks_once_and_keeps_the_pick() {
  let mut agents = Agents::new(true);
  team(&agents);
  let first = claude(&agents.root, "Claude 2", "2.1.173");
  claude(&agents.root, "Claude Home", "2.1.170");
  let (profile, _) = builder(&agents);
  show(&mut agents, profile);
  assert!(agents.exists("team-hub-pick"));
  assert_eq!(agents.text("team-hub-runs-on-sub"), "2 Claude Code connections here");
  assert_eq!(
    agents.text("team-hub-ready-connection-words"),
    "Choose a Claude Code connection here"
  );
  // The connection is the first thing missing, so it is what the bar offers, naming the first listed.
  assert!(agents.text("team-hub-fix-use").contains("Use Claude 2 for Builder"));
  assert_eq!(
    agents.text("team-hub-actions-hint"),
    "Asked once per profile · kept on this computer"
  );
  assert!(agents.exists("team-hub-ready-connection-use"));
  agents.click("team-hub-fix-use");
  agents.until("the pick is kept", |s| {
    ProjectDb::open(&s.root)
      .expect("database")
      .connection_pick(profile)
      .expect("pick")
      .is_some_and(|pick| pick.connection == first)
  });
  // Picked: the connection runs it, and the bar moves on to the next missing part.
  agents.until("the pick is read back", |s| {
    s.exists("team-hub-fix-add-value") && !s.exists("team-hub-fix-use")
  });
  assert_eq!(agents.text("team-hub-runs-on-sub"), "picked on this computer");
  assert_eq!(
    agents.text("team-hub-ready-connection-words"),
    "Claude Code · Claude 2 · claude 2.1.173"
  );
}

#[test]
fn team_hub_profile_with_no_connection_of_its_kind_says_what_to_add_and_substitutes_nothing() {
  let mut agents = Agents::new(true);
  team(&agents);
  let (profile, _) = builder(&agents);
  show(&mut agents, profile);
  assert_eq!(
    agents.text("team-hub-runs-on-none"),
    "No Claude Code connection on this computer"
  );
  assert_eq!(agents.text("team-hub-runs-on-sub"), "nothing is substituted");
  assert_eq!(
    agents.text("team-hub-ready-connection-words"),
    "No Claude Code connection here"
  );
  assert!(agents.text("team-hub-fix-add").contains("Add a Claude Code connection"));
  assert_eq!(agents.text("team-hub-actions-hint"), "Opens Settings › Accounts");
  agents.click("team-hub-fix-add");
  agents.until("Settings › Accounts", |s| {
    s.window.navigation().route_now() == Route::Accounts(AccountsView::default())
  });
  // The project's Codex connection, of another kind, is not substituted for it.
  assert!(!agents.exists("team-hub-pick"));
}

#[test]
fn team_hub_profile_pins_the_fix_of_the_first_missing_part_and_nothing_when_nothing_is_missing() {
  let mut agents = Agents::new(true);
  team(&agents);
  claude(&agents.root, "Claude 2", "2.1.173");
  let (profile, server) = builder(&agents);
  // The server is switched off here: it is missing before the secret, so it is what the bar offers.
  ProjectDb::open(&agents.root)
    .expect("database")
    .set_team_mcp_on_here(server, false)
    .expect("off here");
  show(&mut agents, profile);
  assert_eq!(agents.text("team-hub-readiness-count"), "2 missing");
  assert!(agents.text("team-hub-fix-switch-on").contains("Switch on gh"));
  assert_eq!(agents.text("team-hub-actions-hint"), "Then Add value for GITHUB_TOKEN");
  assert!(!agents.exists("team-hub-fix-add-value"));
  agents.click("team-hub-fix-switch-on");
  agents.until("the server is on and the next part is offered", |s| {
    s.exists("team-hub-fix-add-value") && !s.exists("team-hub-fix-switch-on")
  });
  assert_eq!(agents.text("team-hub-readiness-count"), "1 missing");
  // The secret's value is typed in a dialog, masked, and kept by the secret's name.
  agents.click("team-hub-ready-secret-GITHUB_TOKEN-add-value");
  agents.until("the dialog", |s| s.exists("team-hub-secret-dialog"));
  agents.set("team-hub-secret-value", "ghp_not-a-real-token");
  agents.click("team-hub-secret-save");
  agents.until("the value is kept and nothing is missing", |s| {
    s.exists("team-hub-readiness-count") && s.text("team-hub-readiness-count") == "all set"
  });
  // Nothing to fix: no bar, and nothing disabled in its place.
  assert!(!agents.exists("team-hub-actions"));
  assert!(!agents.exists("team-hub-secret-dialog"));
}
