//! What a computer fills in for a team project, over MCP: the connection a team profile runs on is picked; a team MCP
//! server is turned off but never on, and its program is never set, by automation, whatever *Let automation change
//! agent permissions* says, and without the write that a person's click reaches (the store is left as it was); the
//! secrets list shows names and that a value is set, never a value.

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
};

use orchester_core::{
  blob::ContentHash,
  id::{ConnectionId, ProjectId, ResourceId, ServerId, UserId},
  name::Name,
  project::ProjectKey,
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, ResourceBody, Slug, TeamMcpServer, VersionNumber},
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

use super::{Served, vault::MemoryVault};

const SECRET_VALUE: &str = "fake-value-7Qx2";

/// The team project the tests fill in: Kontur shared on a server, with the team's profiles and MCP servers cached.
struct Team {
  served: Served,
  repo: PathBuf,
  app_database: PathBuf,
  project: ProjectId,
  server: ServerId,
  codex_profile: ResourceId,
  claude_profile: ResourceId,
  program_server: ResourceId,
}

fn team() -> Team {
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
  fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("the project database opens")
  }

  /// How often the project's connections were counted as changed: what the screens read again on.
  fn connection_changes(&self) -> u64 {
    self
      .served
      .services()
      .changes
      .counts(self.project)
      .of(Outside::Connections)
  }

  fn on_here(&self, server: ResourceId) -> bool {
    self.db().team_mcp_on_here(server).expect("read")
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
fn domain_team_mcp_server_program_is_person_only_always_and_changes_nothing() {
  let mut team = team();
  let program = std::env::current_exe()
    .expect("the test program")
    .to_string_lossy()
    .into_owned();
  let set = json!({ "project": "Kontur", "server": "knowledge", "program": program });
  let changes = team.connection_changes();
  for let_automation_widen in [false, true] {
    team.served.let_automation_widen(let_automation_widen);
    assert_person_only(&team.served.refused("update_mcp_server", &set));
    assert!(!team.program_set(team.program_server), "no program was kept");
    assert_eq!(team.connection_changes(), changes);
  }
  // Anything else of a team server is the team's to change.
  let rename = json!({ "project": "Kontur", "server": "knowledge", "name": "other" });
  let refused = team.served.refused("update_mcp_server", &rename);
  assert_eq!(refused["code"], "team_resource", "{refused}");
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
