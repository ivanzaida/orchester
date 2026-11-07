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
  connection::{DecisionConnection, DecisionServerUrl},
  id::{AgentAccountId, ConnectionId, DecisionConnectionId, ProjectId, ResourceId, ServerId, UserId},
  name::Name,
  project::ProjectKey,
  resource::{
    AgentProfile, ModelId, PromptText, ProviderSettings, ResourceBody, Slug, TeamMcpServer, TeamReviewer, VersionNumber,
  },
  secret::{SecretName, SecretValue},
  sharing::{MemberRole, SharedPerson, TaskRevision},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{
  AppDb, CachedVersion, ProjectDb, ReferenceTarget, ResourceSharing, SharingLink, VersionReference,
};
use orchester_workspace::tasks::outside::Outside;
use serde_json::{Value, json};

use super::{Served, accounts::add_account, vault::MemoryVault};

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

fn team() -> Team {
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
  let codex_profile = team_resource(&mut db, "codex-builder", profile(), vec![role("codex_cli")]);
  let claude_profile = team_resource(&mut db, "claude-builder", profile(), vec![role("claude_code_cli")]);
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
  Team {
    served,
    repo: seeded.repo,
    app_database: seeded.app_database,
    project: seeded.project,
    server,
    codex_profile,
    claude_profile,
    program_server,
  }
}

fn name(text: &str) -> Name {
  Name::new(text).expect("valid name")
}

fn profile() -> AgentProfile {
  AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
}

/// The reference a team profile's version has to the CLI kind `role`.
fn role(role: &str) -> VersionReference {
  VersionReference {
    target: ReferenceTarget::Connection(ConnectionId::generate()),
    name: role.to_owned(),
    role: Some(role.to_owned()),
  }
}

fn team_resource<B: ResourceBody>(
  db: &mut ProjectDb,
  slug: &str,
  body: B,
  references: Vec<VersionReference>,
) -> ResourceId {
  let id = db
    .create_resource(
      Slug::new(slug).expect("slug"),
      name(slug),
      &body,
      Timestamp::from_unix_millis(10),
    )
    .expect("resource")
    .id();
  db.set_resource_sharing(
    id,
    Some(ResourceSharing::Shared(TaskRevision::try_from(1).expect("revision"))),
  )
  .expect("shared");
  let version = CachedVersion {
    number: VersionNumber::FIRST,
    published_at: Timestamp::from_unix_millis(20),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Olena".to_owned(),
    },
    server_body: json!({}),
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references,
    local_body: body,
    waits_here: false,
  };
  db.cache_shared_version(id, &version).expect("cached");
  id
}

impl Team {
  pub(super) fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("the project database opens")
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

  fn codex_connection(&mut self) -> String {
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

fn assert_person_only(error: &Value) {
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

#[test]
fn domain_team_pick_reviewer_connection_keeps_the_pick_for_a_team_reviewer() {
  let mut team = team();
  let connection = team.codex_connection();
  let language = |cli: &str| -> TeamReviewer {
    serde_json::from_value(json!({"kind": "language", "cli": cli, "model": "m-1", "effort": null})).expect("language")
  };
  let decision = |destinations: Value| -> TeamReviewer {
    serde_json::from_value(
      json!({"kind": "decision", "model": "d-1", "lowest_threshold": 0.8, "destinations": destinations}),
    )
    .expect("decision")
  };
  let mut db = team.db();
  let codex = team_resource(&mut db, "codex-reviewer", language("codex_cli"), Vec::new());
  let claude = team_resource(&mut db, "claude-reviewer", language("claude_code_cli"), Vec::new());
  let listing = json!({"kind": "hosts", "hosts": ["api.typesafe.ai"]});
  let decisions = team_resource(&mut db, "decisions", decision(listing), Vec::new());
  let members_only = json!({"kind": "members_computers"});
  let members = team_resource(&mut db, "members", decision(members_only), Vec::new());
  let old = team_resource(&mut db, "old", language("codex_cli"), Vec::new());
  make_old_form(&team, old);
  let (local, hosted) = (
    decision_connection(&mut db, "local", "http://127.0.0.1:8000"),
    decision_connection(&mut db, "hosted", "https://api.typesafe.ai/v1"),
  );
  drop(db);
  let args = |reviewer: ResourceId, connection: &str| json!({ "project": "Kontur", "reviewer": reviewer.to_string(), "connection": connection });

  let picked = team.served.ok("pick_reviewer_connection", &args(codex, &connection));
  assert_eq!(picked["reviewer"]["name"], "codex-reviewer");
  assert_eq!(picked["connection"]["id"], connection);
  assert_eq!(
    team
      .db()
      .connection_pick(codex)
      .expect("read")
      .map(|pick| pick.connection.to_string()),
    Some(connection.clone())
  );
  let refused = team
    .served
    .refused("pick_reviewer_connection", &args(claude, &connection));
  assert_eq!(refused["code"], "connection_does_not_fit", "{refused}");
  assert_eq!(team.db().connection_pick(claude).expect("read"), None);
  let refused = team.served.refused(
    "pick_reviewer_connection",
    &json!({"project": "Kontur", "reviewer": "nobody", "connection": connection}),
  );
  assert_eq!(refused["code"], "reviewer_not_found", "{refused}");

  team
    .served
    .ok("pick_reviewer_connection", &args(decisions, &local.to_string()));
  assert_eq!(team.db().decision_pick(decisions).expect("read"), Some(local));
  let refused = team
    .served
    .refused("pick_reviewer_connection", &args(decisions, &hosted.to_string()));
  assert_person_only(&refused);
  assert_eq!(
    team.db().decision_pick(decisions).expect("read"),
    Some(local),
    "the pick stays"
  );
  let refused = team
    .served
    .refused("pick_reviewer_connection", &args(decisions, &connection));
  assert_eq!(refused["code"], "connection_not_found", "{refused}");

  let refused = team
    .served
    .refused("pick_reviewer_connection", &args(members, &hosted.to_string()));
  assert_eq!(refused["code"], "server_not_allowed", "{refused}");
  assert_eq!(team.db().decision_pick(members).expect("read"), None);
  let refused = team.served.refused("pick_reviewer_connection", &args(old, &connection));
  assert_eq!(refused["code"], "not_team_form", "{refused}");
  assert_eq!(team.db().connection_pick(old).expect("read"), None);
}

/// Writes the body of the earlier form (it names a connection) over the newest version of `reviewer`, as a cache from
/// before ORC-887 holds it.
fn make_old_form(team: &Team, reviewer: ResourceId) {
  let old = json!({
    "kind": "language", "connection": {"kind": "connection", "name": "Team language", "role": "codex_cli"},
    "model": "m-1", "effort": null
  });
  let stored = json!({"schema": <TeamReviewer as ResourceBody>::SCHEMA, "body": old}).to_string();
  let database = rusqlite::Connection::open(team.repo.join(".orchester").join("orchester.db")).expect("database");
  let changed = database
    .execute(
      "UPDATE shared_version_bodies SET local_body = ?1 \
       WHERE version_id IN (SELECT id FROM resource_versions WHERE resource_id = ?2)",
      rusqlite::params![stored, reviewer.to_string()],
    )
    .expect("the old form");
  assert_eq!(changed, 1);
}

fn decision_connection(db: &mut ProjectDb, label: &str, url: &str) -> DecisionConnectionId {
  let made = DecisionConnection::new(
    DecisionConnectionId::generate(),
    name(label),
    DecisionServerUrl::new(url).expect("server"),
    None,
  );
  db.create_decision_connection(&made).expect("a decision connection");
  made.id()
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
  let made = team.served.ok("update_mcp_server", &named);
  assert_eq!(made["server"]["launch"]["program"], "uvx", "{made}");
  assert_eq!(made["server"]["published_version"], 2, "{made}");
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
