//! The profile tools and the team MCP server listing say where each resource stands on this computer: `get_profile`
//! and `list_profiles` carry `readiness` (`ready_here`, `group`, the ordered `missing` parts, each with the fix a tool
//! offers), `list_team_mcp_servers` lists the team's servers with theirs, a local project answers it with what a run
//! would find, and no answer holds a program's path or a secret's value.

use std::fs;

use orchester_core::{
  blob::ContentHash,
  connection::{
    AgentConnection, AgentProvider,
    mcp::{McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, StdioServer},
  },
  id::{ConnectionId, ProjectId, ResourceId, UserId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, CommandProgram, ModelId, PromptText, ProviderSettings, ResourceBody, Slug, TeamAgentProfile,
    TeamMcpServer, VersionNumber,
  },
  secret::{SecretName, SecretValue},
  sharing::{SharedPerson, TaskRevision},
  store::{ConnectionStore, McpServerStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{AppDb, CachedVersion, ProjectDb, ResourceSharing};
use serde_json::{Value, json};

use crate::{Checkout, team_server::TeamServer};

const SECRET_VALUE: &str = "fake-value-7Qx2";

fn name(text: &str) -> Name {
  Name::new(text).expect("name")
}

fn team_profile(cli: AgentProvider, servers: &[ResourceId], secrets: &[&str]) -> TeamAgentProfile {
  TeamAgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    (
      cli,
      ModelId::new("gpt-5.5").expect("model"),
      ProviderSettings::default(),
    ),
  )
  .with_mcp_servers(servers.to_vec())
  .expect("distinct servers")
  .with_secrets(
    secrets
      .iter()
      .map(|text| SecretName::new(text).expect("secret name"))
      .collect(),
  )
  .expect("distinct secrets")
}

/// A team resource at v1 as the stream caches it: the server holds `server_body`, this computer `local`.
fn team_resource<B: ResourceBody + Clone>(db: &mut ProjectDb, slug: &str, server_body: Value, local: &B) -> ResourceId {
  let id = db
    .create_resource(
      Slug::new(slug).expect("slug"),
      name(slug),
      local,
      Timestamp::from_unix_millis(10),
    )
    .expect("resource")
    .id();
  db.set_resource_sharing(
    id,
    Some(ResourceSharing::Shared(TaskRevision::try_from(1).expect("revision"))),
  )
  .expect("shared");
  let cached = CachedVersion {
    number: VersionNumber::FIRST,
    published_at: Timestamp::from_unix_millis(20),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Olena".to_owned(),
    },
    server_body,
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references: Vec::new(),
    local_body: local.clone(),
  };
  db.cache_shared_version(id, &cached).expect("cached");
  id
}

fn server_resource(db: &mut ProjectDb, body: Value) -> ResourceId {
  let server: TeamMcpServer = serde_json::from_value(body.clone()).expect("a team MCP server");
  team_resource(db, "knowledge", body, &server)
}

fn profile_resource(db: &mut ProjectDb, team: &TeamAgentProfile) -> ResourceId {
  let local = AgentProfile::from_team(team, ConnectionId::generate());
  team_resource(db, "builder", serde_json::to_value(team).expect("json"), &local)
}

fn program_server() -> Value {
  json!({"name": "knowledge", "launch": {"kind": "program", "program": "npx", "variables": ["API_TOKEN"]}})
}

/// A team whose Builder profile chooses the team's `knowledge` MCP server, which needs the team secret `API_TOKEN`.
fn team_with_server() -> (TeamServer, Checkout, ResourceId, ResourceId) {
  let server = TeamServer::start(crate::fixture::Config::new("Say hello."));
  let mut ids = None;
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    let knowledge = server_resource(db, program_server());
    let profile = profile_resource(db, &team_profile(AgentProvider::CodexCli, &[knowledge], &["API_TOKEN"]));
    ids = Some((profile, knowledge));
  });
  let (profile, knowledge) = ids.expect("prepared");
  (server, checkout, profile, knowledge)
}

fn kontur(mut args: Value) -> Value {
  args["project"] = json!("Kontur");
  args
}

fn profile_readiness(checkout: &Checkout, profile: ResourceId) -> Value {
  let read = checkout
    .call("get_profile", &kontur(json!({"profile": profile.to_string()})))
    .expect("get_profile");
  read["profile"]["readiness"].clone()
}

fn listed_server(checkout: &Checkout) -> Value {
  let read = checkout
    .call("list_team_mcp_servers", &kontur(json!({})))
    .expect("list_team_mcp_servers");
  let servers = read["mcp_servers"].as_array().expect("servers");
  assert_eq!(servers.len(), 1, "{read}");
  servers[0].clone()
}

fn kinds(readiness: &Value) -> Vec<String> {
  readiness["missing"]
    .as_array()
    .expect("missing parts")
    .iter()
    .map(|part| part["kind"].as_str().expect("a kind").to_owned())
    .collect()
}

fn add_connection(checkout: &Checkout, provider: AgentProvider, label: &str) {
  let made = AgentConnection::new(
    ConnectionId::generate(),
    name(label),
    provider,
    checkout.root().join(format!("{label}.exe")),
    Timestamp::now(),
  )
  .expect("connection");
  checkout.db().create_connection(&made).expect("a connection");
}

fn program_file(checkout: &Checkout) -> std::path::PathBuf {
  let path = checkout.root().join("knowledge-mcp");
  fs::write(&path, "#!/bin/sh\n").expect("a program file");
  path
}

fn set_program(checkout: &Checkout, server: ResourceId, path: &std::path::Path) {
  let link = checkout.db().sharing().expect("read").expect("shared");
  AppDb::open(&checkout._data.path().join("orchester.db"))
    .expect("the application database")
    .set_mcp_server_program(link.server, server, path, Timestamp::now())
    .expect("the program");
}

#[test]
fn team_mcp_readiness_a_profile_that_needs_nothing_more_is_ready_here() {
  let server = TeamServer::start(crate::fixture::Config::new("Say hello."));
  let mut profile = None;
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    profile = Some(profile_resource(db, &team_profile(AgentProvider::CodexCli, &[], &[])));
  });
  let id = profile.expect("prepared");

  let readiness = profile_readiness(&checkout, id);
  assert_eq!(readiness["ready_here"], true, "{readiness}");
  assert_eq!(readiness["group"], "ready");
  assert_eq!(readiness["version"], 1);
  assert_eq!(readiness["missing"], json!([]));

  let listed = checkout
    .call("list_profiles", &kontur(json!({})))
    .expect("list_profiles");
  let entry = &listed["profiles"][0];
  assert_eq!(entry["readiness"], readiness, "{listed}");
}

#[test]
fn team_mcp_readiness_a_profile_with_two_connections_of_its_kind_needs_a_pick_until_one_is_made() {
  let (_server, checkout, profile, _) = team_with_server();
  add_connection(&checkout, AgentProvider::CodexCli, "second-codex");

  let readiness = profile_readiness(&checkout, profile);
  let first = &readiness["missing"][0];
  assert_eq!(first["kind"], "connection", "{readiness}");
  assert_eq!(first["problem"], "pick_needed");
  assert_eq!(first["cli"], "codex_cli");
  assert_eq!(first["fix"], json!({"tool": "pick_profile_connection", "gate": null}));

  let connections = checkout
    .call("list_connections", &kontur(json!({})))
    .expect("connections");
  let chosen = connections["agents"][0]["id"].as_str().expect("a connection");
  let args = kontur(json!({"profile": profile.to_string(), "connection": chosen}));
  checkout.call("pick_profile_connection", &args).expect("picked");
  let readiness = profile_readiness(&checkout, profile);
  assert!(!kinds(&readiness).contains(&"connection".to_owned()), "{readiness}");
}

#[test]
fn team_mcp_readiness_a_profile_of_a_cli_this_computer_lacks_needs_a_connection_added() {
  let server = TeamServer::start(crate::fixture::Config::new("Say hello."));
  let mut profile = None;
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    profile = Some(profile_resource(
      db,
      &team_profile(AgentProvider::ClaudeCodeCli, &[], &[]),
    ));
  });

  let readiness = profile_readiness(&checkout, profile.expect("prepared"));
  assert_eq!(readiness["ready_here"], false, "{readiness}");
  assert_eq!(readiness["group"], "needs_you_here");
  let part = &readiness["missing"][0];
  assert_eq!(
    (part["kind"].as_str(), part["problem"].as_str()),
    (Some("connection"), Some("none_of_kind"))
  );
  assert_eq!(part["cli"], "claude_code_cli");
  assert_eq!(part["fix"]["tool"], "add_cli_connection");
}

#[test]
fn team_mcp_readiness_a_server_nobody_decided_on_is_missing_on_and_the_profile_that_chooses_it_too() {
  let (_server, checkout, profile, knowledge) = team_with_server();

  let readiness = profile_readiness(&checkout, profile);
  assert_eq!(readiness["group"], "needs_you_here", "{readiness}");
  assert_eq!(kinds(&readiness), ["mcp_off", "mcp_program", "secret"]);
  let off = &readiness["missing"][0];
  assert_eq!(off["server"], json!({"id": knowledge.to_string(), "name": "knowledge"}));
  assert_eq!(off["fix"], json!({"tool": "set_mcp_server_enabled", "gate": "person"}));

  let server = listed_server(&checkout);
  assert_eq!(server["id"], knowledge.to_string());
  assert_eq!(server["name"], "knowledge");
  assert_eq!(server["version"], 1);
  assert_eq!(server["launch"]["kind"], "program");
  assert_eq!(server["readiness"]["group"], "needs_you_here", "{server}");
  assert_eq!(kinds(&server["readiness"]), ["mcp_off", "mcp_program", "secret"]);
}

#[test]
fn team_mcp_readiness_a_server_switched_off_is_off_here_and_turning_it_on_stays_a_persons() {
  let (_server, checkout, profile, knowledge) = team_with_server();
  let off = kontur(json!({"server": knowledge.to_string(), "enabled": false}));
  checkout.call("set_mcp_server_enabled", &off).expect("switched off");

  let server = listed_server(&checkout);
  assert_eq!(server["readiness"]["group"], "off_here", "{server}");
  assert_eq!(server["readiness"]["ready_here"], false);
  assert_eq!(profile_readiness(&checkout, profile)["group"], "needs_you_here");

  let on = kontur(json!({"server": knowledge.to_string(), "enabled": true}));
  assert_eq!(checkout.refused("set_mcp_server_enabled", &on), "person_only");
}

#[test]
fn team_mcp_readiness_a_program_that_is_not_set_or_whose_file_is_gone_is_told_apart() {
  let (_server, checkout, profile, knowledge) = team_with_server();
  checkout
    .db()
    .set_team_mcp_on_here(knowledge, true)
    .expect("switched on");

  let missing = profile_readiness(&checkout, profile);
  let program = &missing["missing"][0];
  assert_eq!(
    (program["kind"].as_str(), program["problem"].as_str()),
    (Some("mcp_program"), Some("not_set"))
  );
  assert_eq!(program["fix"], json!({"tool": null, "gate": "person"}));

  let file = program_file(&checkout);
  set_program(&checkout, knowledge, &file);
  let set = profile_readiness(&checkout, profile);
  assert_eq!(kinds(&set), ["secret"], "{set}");

  fs::remove_file(&file).expect("the file goes");
  let gone = listed_server(&checkout);
  assert_eq!(gone["readiness"]["missing"][0]["problem"], "file_gone", "{gone}");
  assert!(
    !gone.to_string().contains(&file.display().to_string()),
    "no program path is listed: {gone}"
  );
}

#[test]
fn team_mcp_readiness_a_secret_without_a_value_is_named_and_ready_once_it_has_one_never_shown() {
  let (_server, checkout, profile, knowledge) = team_with_server();
  checkout
    .db()
    .set_team_mcp_on_here(knowledge, true)
    .expect("switched on");
  set_program(&checkout, knowledge, &program_file(&checkout));

  let readiness = profile_readiness(&checkout, profile);
  assert_eq!(
    readiness["missing"],
    json!([{
      "kind": "secret", "name": "API_TOKEN", "fix": {"tool": null, "gate": "person"}
    }])
  );

  let value = SecretValue::new(SECRET_VALUE.to_owned()).expect("value");
  let name = SecretName::new("API_TOKEN").expect("name");
  checkout
    .db()
    .set_team_secret_value(&*checkout.vault, &name, &value)
    .expect("the value");
  let ready = profile_readiness(&checkout, profile);
  assert_eq!(ready["ready_here"], true, "{ready}");
  let everything = [
    checkout
      .call("get_profile", &kontur(json!({"profile": profile.to_string()})))
      .expect("profile"),
    checkout.call("list_profiles", &kontur(json!({}))).expect("profiles"),
    checkout
      .call("list_team_mcp_servers", &kontur(json!({})))
      .expect("servers"),
  ];
  assert!(everything.iter().all(|read| !read.to_string().contains(SECRET_VALUE)));
  assert_eq!(listed_server(&checkout)["readiness"]["group"], "ready");
}

#[test]
fn team_mcp_readiness_a_profile_version_that_is_not_in_the_teams_form_is_undecided() {
  let server = TeamServer::start(crate::fixture::Config::new("Say hello."));
  let mut profile = None;
  let checkout = Checkout::prepared(Some(&server), &[], |db| {
    let team = team_profile(AgentProvider::CodexCli, &[], &[]);
    let local = AgentProfile::from_team(&team, ConnectionId::generate());
    profile = Some(team_resource(db, "builder", json!({}), &local));
  });

  let readiness = profile_readiness(&checkout, profile.expect("prepared"));
  assert_eq!(readiness["ready_here"], false, "{readiness}");
  assert_eq!(readiness["group"], "needs_you_here");
  assert_eq!(
    readiness["missing"],
    json!([{"kind": "undecided", "why": "not_team_form", "fix": {"tool": null, "gate": null}}])
  );
}

fn local_server(program: &str) -> McpServer {
  McpServer::new(
    ConnectionId::generate(),
    McpServerName::new("pencil-design").expect("name"),
    McpTransport::Stdio(StdioServer::new(
      CommandProgram::new(program).expect("program"),
      vec![],
      None,
    )),
    McpEnvironment::default(),
    Timestamp::now(),
  )
  .expect("server")
}

/// A published profile of a local project that chooses a server whose program no computer has: the profile and the
/// server.
fn local_profile_with_missing_program(db: &mut ProjectDb) -> (ResourceId, ConnectionId) {
  let server = local_server("orchester-test-no-such-program");
  db.create_mcp_server(&server).expect("a server");
  let connection = db.connections().expect("connections")[0].id();
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    connection,
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
  .with_mcp_servers(McpServerSelection::new(vec![server.id()]).expect("one server"));
  let id = db
    .create_resource(
      Slug::new("builder").expect("slug"),
      name("builder"),
      &profile,
      Timestamp::now(),
    )
    .expect("resource")
    .id();
  db.publish_draft::<AgentProfile>(id, Timestamp::now())
    .expect("published");
  (id, server.id())
}

#[test]
fn team_mcp_readiness_a_local_project_says_what_a_run_would_find_and_lists_no_team_servers() {
  let mut ids = None;
  let checkout = Checkout::prepared(None, &[], |db| ids = Some(local_profile_with_missing_program(db)));
  let (profile, server) = ids.expect("prepared");

  let readiness = profile_readiness(&checkout, profile);
  assert_eq!(readiness["ready_here"], false, "{readiness}");
  assert_eq!(
    readiness["missing"],
    json!([{
      "kind": "local_mcp_program",
      "server": {"id": server.to_string(), "name": "pencil-design"},
      "problem": "not_found",
      "fix": {"tool": "update_mcp_server", "gate": "ap"},
    }])
  );

  let off = kontur(json!({"server": server.to_string(), "enabled": false}));
  checkout.call("set_mcp_server_enabled", &off).expect("turned off");
  assert_eq!(profile_readiness(&checkout, profile)["ready_here"], true);

  let servers = checkout
    .call("list_team_mcp_servers", &kontur(json!({})))
    .expect("servers");
  assert_eq!(servers["mcp_servers"], json!([]));
}

/// A second project, a local one named Local, registered in the checkout's workspace.
fn add_local_project(checkout: &Checkout) -> (ResourceId, ConnectionId) {
  let root = checkout._data.path().join("local-project");
  fs::create_dir(&root).expect("a folder");
  crate::repository(&root);
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("LOC").expect("key"),
    name("Local"),
  );
  let mut db = ProjectDb::create(&root, project).expect("create project");
  let codex = AgentConnection::new(
    ConnectionId::generate(),
    name("Codex"),
    AgentProvider::CodexCli,
    root.join("codex.exe"),
    Timestamp::now(),
  )
  .expect("connection");
  db.create_connection(&codex).expect("a connection");
  let ids = local_profile_with_missing_program(&mut db);
  drop(db);
  checkout.workspace.open(&root).expect("register the project");
  ids
}

#[test]
fn team_mcp_readiness_without_a_project_each_project_answers_its_own() {
  let (_server, checkout, team_profile, knowledge) = team_with_server();
  let (local_profile, local_server) = add_local_project(&checkout);

  let profiles = checkout.call("list_profiles", &json!({})).expect("list_profiles");
  assert_eq!(profiles["unreadable_projects"], json!([]), "{profiles}");
  let readiness_of = |project: &str, id: ResourceId| {
    let found: Vec<_> = profiles["profiles"]
      .as_array()
      .expect("profiles")
      .iter()
      .filter(|entry| entry["project"]["name"] == project && entry["id"] == id.to_string())
      .collect();
    assert_eq!(found.len(), 1, "{project}: {profiles}");
    found[0]["readiness"].clone()
  };
  let team = readiness_of("Kontur", team_profile);
  assert_eq!(team["group"], "needs_you_here", "{team}");
  assert!(kinds(&team).contains(&"mcp_off".to_owned()), "{team}");
  let local = readiness_of("Local", local_profile);
  assert_eq!(local["ready_here"], false, "{local}");
  assert_eq!(kinds(&local), ["local_mcp_program"], "{local}");
  assert_eq!(local["missing"][0]["server"]["id"], local_server.to_string());

  let servers = checkout
    .call("list_team_mcp_servers", &json!({}))
    .expect("list_team_mcp_servers");
  assert_eq!(servers["unreadable_projects"], json!([]), "{servers}");
  let listed = servers["mcp_servers"].as_array().expect("servers");
  assert_eq!(listed.len(), 1, "only the team project has team servers: {servers}");
  assert_eq!(listed[0]["project"]["name"], "Kontur");
  assert_eq!(listed[0]["id"], knowledge.to_string());
  assert_eq!(listed[0]["readiness"]["group"], "needs_you_here");
}

#[test]
fn team_mcp_readiness_a_profile_without_a_published_version_has_none() {
  let mut draft = None;
  let checkout = Checkout::prepared(None, &[], |db| {
    let connection = db.connections().expect("connections")[0].id();
    let profile = AgentProfile::new(
      name("Builder"),
      String::new(),
      PromptText::new("Build.").expect("prompt"),
      connection,
      ModelId::new("gpt-5.5").expect("model"),
      ProviderSettings::default(),
    );
    draft = Some(
      db.create_resource(
        Slug::new("builder").expect("slug"),
        name("builder"),
        &profile,
        Timestamp::now(),
      )
      .expect("resource")
      .id(),
    );
  });
  let id = draft.expect("prepared");

  assert_eq!(profile_readiness(&checkout, id), Value::Null);
  let listed = checkout
    .call("list_profiles", &kontur(json!({})))
    .expect("list_profiles");
  assert_eq!(listed["profiles"][0]["id"], id.to_string(), "{listed}");
  assert_eq!(listed["profiles"][0]["readiness"], Value::Null, "{listed}");
}

#[test]
fn team_mcp_readiness_a_local_profile_whose_connection_is_gone_is_fixed_in_its_draft() {
  let mut gone = None;
  let checkout = Checkout::prepared(None, &[], |db| {
    let profile = AgentProfile::new(
      name("Builder"),
      String::new(),
      PromptText::new("Build.").expect("prompt"),
      ConnectionId::generate(),
      ModelId::new("gpt-5.5").expect("model"),
      ProviderSettings::default(),
    );
    let id = db
      .create_resource(
        Slug::new("builder").expect("slug"),
        name("builder"),
        &profile,
        Timestamp::now(),
      )
      .expect("resource")
      .id();
    db.publish_draft::<AgentProfile>(id, Timestamp::now())
      .expect("published");
    gone = Some(id);
  });

  let readiness = profile_readiness(&checkout, gone.expect("prepared"));
  assert_eq!(readiness["ready_here"], false, "{readiness}");
  assert_eq!(readiness["missing"][0]["why"], "connection_gone", "{readiness}");
  assert_eq!(
    readiness["missing"][0]["fix"],
    json!({"tool": "save_profile_draft", "gate": null})
  );
}
