//! What a computer fills in for a team project, over MCP: the connection a team profile runs on is picked; a team MCP
//! server is turned off but never on, and its program is never set, by automation, whatever *Let automation change
//! agent permissions* says, and without the write that a person's click reaches (the store is left as it was); the
//! secrets list shows names and that a value is set, never a value.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
};

use orchester_core::{
  account::AccountCli,
  blob::ContentHash,
  connection::AgentProvider,
  id::{AgentAccountId, ConnectionId, ProjectId, ResourceId, ServerId, UserId},
  name::Name,
  project::ProjectKey,
  resource::{
    AgentProfile, ModelId, PromptText, ProviderSettings, Resource, ResourceBody, Slug, TeamAgentProfile, TeamMcpServer,
    VersionNumber,
  },
  secret::{SecretName, SecretValue},
  sharing::{MemberRole, SharedPerson, TaskRevision},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{AppDb, CachedVersion, ProjectDb, ReferenceTarget, SharingLink};
use orchester_workspace::tasks::outside::Outside;
use serde_json::{Value, json};

use super::{Served, accounts::add_account, team_cache, vault::MemoryVault};

const SECRET_VALUE: &str = "fake-value-7Qx2";

/// The team project the tests fill in: Kontur shared on a server, with the team's profiles and MCP servers cached.
pub(super) struct Team {
  pub(super) served: Served,
  pub(super) repo: PathBuf,
  pub(super) app_database: PathBuf,
  pub(super) project: ProjectId,
  pub(super) server: ServerId,
  codex_profile: ResourceId,
  claude_profile: ResourceId,
  pub(super) program_server: ResourceId,
}

pub(super) fn team() -> Team {
  team_as(MemberRole::Owner)
}

/// The team project as a person with the role `standing` in it sees it.
pub(super) fn team_as(standing: MemberRole) -> Team {
  let codex = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
  let (served, seeded) = Served::seeded_with("empty", Some(codex));
  let server = ServerId::generate();
  let mut db = ProjectDb::open(&seeded.repo).expect("the project database opens");
  db.start_sharing(
    &SharingLink {
      server,
      project: ProjectId::generate(),
      key: ProjectKey::new("KON").expect("key"),
      role: Some(MemberRole::Owner),
      joined_at: Timestamp::from_unix_millis(100),
      cursor: 0,
      last_synced_at: Timestamp::from_unix_millis(100),
      removed_at: None,
      address: None,
    },
    &[],
    (&[], &HashMap::new()),
  )
  .expect("shared");
  let codex_profile = team_profile(&mut db, "codex-builder", AgentProvider::CodexCli);
  let claude_profile = team_profile(&mut db, "claude-builder", AgentProvider::ClaudeCodeCli);
  let program_server = team_resource(
    &mut db,
    "knowledge",
    serde_json::from_value::<TeamMcpServer>(json!({"name":"knowledge","launch":{"kind":"program","program":"npx"}}))
      .expect("a team MCP server"),
    Vec::new(),
  );
  // The team's resources were set up as its owner; the person then has the role the test asks for.
  db.record_sharing_state(Some(standing), None, None).expect("the role");
  drop(db);
  let team = Team {
    served,
    repo: seeded.repo,
    app_database: seeded.app_database,
    project: seeded.project,
    server,
    codex_profile,
    claude_profile,
    program_server,
  };
  team.keep_team();
  team
}

pub(super) fn name(text: &str) -> Name {
  Name::new(text).expect("valid name")
}

/// The team's profile `slug`, run by the CLI `provider`, at v1: the server's body in the team's form, which names the
/// CLI kind, and a local body that names no connection of this computer.
fn team_profile(db: &mut ProjectDb, slug: &str, provider: AgentProvider) -> ResourceId {
  let team = TeamAgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    (
      provider,
      ModelId::new("gpt-5.5").expect("model"),
      ProviderSettings::default(),
    ),
  );
  let local = AgentProfile::from_team(&team, ConnectionId::generate());
  let server_body = serde_json::to_value(&team).expect("json");
  cache_team(db, slug, (server_body, local), vec![ReferenceTarget::CliKind(provider)])
}

/// The team's resource `slug` at v1, whose body is the team's form as the server sent it.
pub(super) fn team_resource<B: ResourceBody>(
  db: &mut ProjectDb,
  slug: &str,
  body: B,
  references: Vec<ReferenceTarget>,
) -> ResourceId {
  let server_body = serde_json::to_value(&body).expect("json");
  cache_team(db, slug, (server_body, body), references)
}

/// Caches the team's resource `slug` at v1 as the stream caches it: the server's body, and the local body this computer
/// runs it by.
fn cache_team<B: ResourceBody>(
  db: &mut ProjectDb,
  slug: &str,
  (server_body, body): (Value, B),
  references: Vec<ReferenceTarget>,
) -> ResourceId {
  // As a computer that syncs the team has it: the server's id, no draft of its own.
  let resource = Resource::new(
    ResourceId::generate(),
    B::KIND,
    Slug::new(slug).expect("slug"),
    name(slug),
    Timestamp::from_unix_millis(10),
  );
  let id = resource.id();
  db.keep_shared_resource(&resource, TaskRevision::try_from(1).expect("revision"))
    .expect("shared");
  let version = CachedVersion {
    number: VersionNumber::FIRST,
    published_at: Timestamp::from_unix_millis(20),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Olena".to_owned(),
    },
    server_body,
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references,
    local_body: body,
  };
  db.cache_shared_version(id, &version).expect("cached");
  id
}

impl Team {
  pub(super) fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("the project database opens")
  }

  /// Keeps the team's resources cached in the project's database in the app's memory, as its stream would once it
  /// read them from the team server, which these tests do not run.
  pub(super) fn keep_team(&self) {
    team_cache::keep_cached_team(&self.served.services().workspace, &self.repo);
  }

  /// How often the project's connections were counted as changed: what the screens read again on.
  pub(super) fn connection_changes(&self) -> u64 {
    self
      .served
      .services()
      .changes
      .counts(self.project)
      .of(Outside::Connections)
  }

  fn on_here(&self, server: ResourceId) -> bool {
    self.db().team_mcp_switch(server).expect("read") == Some(true)
  }

  fn program_set(&self, server: ResourceId) -> bool {
    AppDb::open(&self.app_database)
      .expect("the application database opens")
      .mcp_server_program(self.server, server)
      .expect("read")
      .is_some()
  }

  pub(super) fn codex_connection(&mut self) -> String {
    let listed = self.served.ok("list_connections", &json!({ "project": "Kontur" }));
    listed["agents"]
      .as_array()
      .and_then(|connections| connections.iter().find(|c| c["provider"] == "codex_cli"))
      .map(|connection| connection["id"].as_str().expect("an id").to_owned())
      .unwrap_or_else(|| panic!("a Codex connection is listed: {listed}"))
  }
}

fn enable(server: ResourceId, enabled: bool) -> Value {
  json!({ "project": "Kontur", "server": server.to_string(), "enabled": enabled })
}

pub(super) fn assert_person_only(error: &Value) {
  assert_eq!(error["code"], "person_only", "{error}");
  assert!(
    error["reasons"].as_array().is_some_and(|reasons| reasons.len() > 1),
    "{error}"
  );
}

#[test]
fn domain_team_pick_profile_connection_keeps_the_pick_for_a_team_profile() {
  let mut team = team();
  let connection = team.codex_connection();
  let args =
    |profile: ResourceId| json!({ "project": "Kontur", "profile": profile.to_string(), "connection": connection });
  let changes = team.connection_changes();
  let picked = team.served.ok("pick_profile_connection", &args(team.codex_profile));
  assert_eq!(picked["profile"]["name"], "codex-builder");
  assert_eq!(picked["connection"]["id"], connection);
  let kept = team
    .db()
    .connection_pick(team.codex_profile)
    .expect("read")
    .expect("a pick");
  assert_eq!(kept.connection.to_string(), connection);
  assert_eq!(kept.account, None);
  assert!(
    team.connection_changes() > changes,
    "the screens read the connections again"
  );

  let refused = team
    .served
    .refused("pick_profile_connection", &args(team.claude_profile));
  assert_eq!(refused["code"], "connection_does_not_fit", "{refused}");
  assert_eq!(team.db().connection_pick(team.claude_profile).expect("read"), None);

  let unknown = json!({ "project": "Kontur", "profile": "nobody", "connection": connection });
  let refused = team.served.refused("pick_profile_connection", &unknown);
  assert_eq!(refused["code"], "profile_not_found", "{refused}");
}

/// The account the codex profile's pick keeps, `None` before any pick.
fn picked_account(team: &Team) -> Option<AgentAccountId> {
  team
    .db()
    .connection_pick(team.codex_profile)
    .expect("read")
    .and_then(|pick| pick.account)
}

#[test]
fn domain_team_pick_profile_connection_account_other_than_the_effective_one_is_a_persons_to_choose() {
  let mut team = team();
  let connection = team.codex_connection();
  let (client, other) = (
    add_account(&team.app_database, AccountCli::Codex, "Client"),
    add_account(&team.app_database, AccountCli::Codex, "Other"),
  );
  let pick = |account: &str| json!({ "project": "Kontur", "profile": team.codex_profile.to_string(), "connection": connection, "account": account });

  // The connection runs on the CLI's Default, so Client is another account: a person's choice.
  team.served.let_automation_widen(false);
  let refused = team.served.refused("pick_profile_connection", &pick("Client"));
  assert_person_only(&refused);
  assert_eq!(
    team.db().connection_pick(team.codex_profile).expect("read"),
    None,
    "no pick was kept"
  );

  team.served.let_automation_widen(true);
  let picked = team.served.ok("pick_profile_connection", &pick("Client"));
  assert_eq!(picked["account"], client.id().to_string());
  assert_eq!(picked_account(&team), Some(client.id()));

  // The same account again, and the pick's account is the one the profile runs on now: no widening.
  team.served.let_automation_widen(false);
  team.served.ok("pick_profile_connection", &pick("client"));
  assert_eq!(picked_account(&team), Some(client.id()));

  // Another account than the earlier pick's is refused again, and the pick stays.
  assert_person_only(&team.served.refused("pick_profile_connection", &pick("Other")));
  assert_eq!(picked_account(&team), Some(client.id()));
  assert_ne!(client.id(), other.id());
}

#[test]
fn domain_team_pick_profile_connection_account_of_the_connection_itself_is_not_gated() {
  let mut team = team();
  let connection = team.codex_connection();
  team.served.let_automation_widen(false);
  let args = json!({
    "project": "Kontur", "profile": team.codex_profile.to_string(), "connection": connection, "account": "default",
  });
  team.served.ok("pick_profile_connection", &args);
  assert_eq!(picked_account(&team), Some(AccountCli::Codex.default_account()));
}

#[test]
fn domain_team_pick_profile_connection_refuses_an_account_that_is_not_of_the_connections_cli() {
  let mut team = team();
  let connection = team.codex_connection();
  add_account(&team.app_database, AccountCli::ClaudeCode, "Lab");
  team.served.let_automation_widen(true);
  for account in ["Lab", "nobody"] {
    let args = json!({
      "project": "Kontur", "profile": team.codex_profile.to_string(), "connection": connection, "account": account,
    });
    let refused = team.served.refused("pick_profile_connection", &args);
    assert_eq!(refused["code"], "account_not_found", "{refused}");
    assert_eq!(team.db().connection_pick(team.codex_profile).expect("read"), None);
  }
}

#[test]
fn domain_team_pick_profile_connection_refuses_a_project_that_is_not_the_teams() {
  let (mut served, _) = Served::seeded_with("empty", Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")));
  let listed = served.ok("list_connections", &json!({ "project": "Kontur" }));
  let connection = listed["agents"][0]["id"].as_str().expect("an id").to_owned();
  let args = json!({ "project": "Kontur", "profile": "anyone", "connection": connection });
  let refused = served.refused("pick_profile_connection", &args);
  assert_eq!(refused["code"], "team_project_required", "{refused}");
}

#[test]
fn domain_team_mcp_server_on_is_person_only_always_and_changes_nothing() {
  let mut team = team();
  let (server, changes) = (team.program_server, team.connection_changes());
  for let_automation_widen in [false, true] {
    team.served.let_automation_widen(let_automation_widen);
    let error = team.served.refused("set_mcp_server_enabled", &enable(server, true));
    assert_person_only(&error);
    assert!(!team.on_here(server), "the server stays off here");
    assert_eq!(team.connection_changes(), changes, "nothing was written or counted");
  }
  // By name too.
  let by_name = json!({ "project": "Kontur", "server": "knowledge", "enabled": true });
  assert_person_only(&team.served.refused("set_mcp_server_enabled", &by_name));
  assert!(!team.on_here(server));
}

#[test]
fn domain_team_mcp_server_off_is_taken_and_counted() {
  let mut team = team();
  team
    .db()
    .set_team_mcp_on_here(team.program_server, true)
    .expect("on here");
  let changes = team.connection_changes();
  let off = team
    .served
    .ok("set_mcp_server_enabled", &enable(team.program_server, false));
  assert_eq!(off["server"]["enabled"], false);
  assert_eq!(off["server"]["team"], true);
  assert!(!team.on_here(team.program_server));
  assert!(team.connection_changes() > changes);
  // Off again is taken too: nothing was on.
  team
    .served
    .ok("set_mcp_server_enabled", &enable(team.program_server, false));
}

#[test]
fn domain_team_mcp_server_program_name_widens_and_its_path_here_is_never_set_by_a_tool() {
  let mut team = team();
  let path = std::env::current_exe()
    .expect("the test program")
    .to_string_lossy()
    .into_owned();
  let named = json!({ "project": "Kontur", "server": "knowledge", "program": "uvx" });
  let located = json!({ "project": "Kontur", "server": "knowledge", "program": path });
  let changes = team.connection_changes();
  for let_automation_widen in [false, true] {
    team.served.let_automation_widen(let_automation_widen);
    let refused = team.served.refused("update_mcp_server", &located);
    assert_eq!(refused["code"], "team_path_refused", "{refused}");
  }
  assert_eq!(team.connection_changes(), changes, "a path changed nothing");

  // A name is the team's data, which widens: a person's unless they let automation change agent permissions.
  team.served.let_automation_widen(false);
  assert_person_only(&team.served.refused("update_mcp_server", &named));
  assert_eq!(team.connection_changes(), changes);

  team.served.let_automation_widen(true);
  let sent = team.served.refused("update_mcp_server", &named);
  assert_eq!(
    sent["code"], "team_server_not_reached",
    "allowed, it gets as far as the team server, which this project's link names none of: {sent}"
  );
  assert!(
    !team.program_set(team.program_server),
    "no tool sets the path on this computer"
  );
}

#[test]
fn domain_team_list_secrets_shows_names_and_that_a_value_is_set_never_the_value() {
  let mut team = team();
  let listed = team.served.ok("list_secrets", &json!({ "project": "Kontur" }));
  assert_eq!(listed["secrets"], json!([]));
  let vault = MemoryVault::default();
  team
    .db()
    .set_team_secret_value(
      &vault,
      &SecretName::new("SENTRY_TOKEN").expect("a name"),
      &SecretValue::new(SECRET_VALUE.to_owned()).expect("a value"),
    )
    .expect("the value is kept");
  let listed = team.served.ok("list_secrets", &json!({ "project": "Kontur" }));
  let secrets = listed["secrets"].as_array().expect("a list of secrets");
  assert_eq!(secrets.len(), 1, "{listed}");
  assert_eq!(secrets[0]["name"], "SENTRY_TOKEN");
  assert_eq!(secrets[0]["team"], true);
  assert_eq!(secrets[0]["value_set"], true);
  assert!(!listed.to_string().contains(SECRET_VALUE), "no value is listed");
}

/// What an MCP client reads of a team project's profiles is what the window shows: the connection this computer picked
/// for a team profile (ORC-848 D6), a profile as the server has it (not a draft beside v1, D7), and nothing of this
/// computer's own that the team never published.
#[test]
fn domain_team_profiles_report_this_computers_pick_and_leave_out_a_local_draft() {
  let mut team = team();
  let connection = team.codex_connection();
  let mut db = team.db();
  let designer = AgentProfile::new(
    name("Designer"),
    String::new(),
    PromptText::new("Design.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  );
  let local = db
    .create_resource(
      Slug::new("designer").expect("slug"),
      name("Designer"),
      &designer,
      Timestamp::from_unix_millis(30),
    )
    .expect("a profile of this computer's own, never published")
    .id();
  drop(db);
  let args = json!({ "project": "Kontur", "profile": team.codex_profile.to_string(), "connection": connection });
  team.served.ok("pick_profile_connection", &args);

  let listed = team.served.ok("list_profiles", &json!({ "project": "Kontur" }));
  let profiles = listed["profiles"].as_array().expect("profiles");
  let ids: Vec<&str> = profiles.iter().filter_map(|profile| profile["id"].as_str()).collect();
  assert!(
    !ids.contains(&local.to_string().as_str()),
    "a local draft is not the team's: {listed}"
  );
  let codex = profiles
    .iter()
    .find(|profile| profile["id"] == team.codex_profile.to_string())
    .unwrap_or_else(|| panic!("the team's profile is listed: {listed}"));
  let picked = team.served.ok("list_connections", &json!({ "project": "Kontur" }));
  let name_of = picked["agents"]
    .as_array()
    .and_then(|all| all.iter().find(|c| c["id"] == connection.as_str()))
    .map(|c| c["name"].clone())
    .expect("the picked connection is listed");
  assert_eq!(codex["connection"], name_of, "the pick is reported: {codex}");
  assert_eq!(
    codex["state"]["state"], "published",
    "the server's version, no draft: {codex}"
  );
  assert_eq!(codex["state"]["published_version"], 1);

  let got = team.served.ok(
    "get_profile",
    &json!({ "project": "Kontur", "profile": team.codex_profile.to_string() }),
  );
  assert_eq!(got["profile"]["connection"]["id"], connection.as_str(), "{got}");
  let unpicked = team.served.ok(
    "get_profile",
    &json!({ "project": "Kontur", "profile": team.claude_profile.to_string() }),
  );
  assert!(
    unpicked["profile"]["connection"].is_null(),
    "no pick, none reported: {unpicked}"
  );
}
