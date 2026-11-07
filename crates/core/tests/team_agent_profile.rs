//! The team's agent profile body (`docs/design/team-configuration.md`): it names a CLI kind, team resources by server
//! id and secrets by name, and nothing of a computer; a computer's profile becomes one only if every id it holds has a
//! team counterpart.

use std::collections::BTreeMap;

use orchester_core::{
  connection::{
    AgentEndpoint, AgentProvider, Connection, ConnectionKind, GitHubHost,
    mcp::{
      HttpServer, McpAuthorization, McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, McpUrl,
    },
  },
  id::{ConnectionId, ResourceId},
  name::Name,
  permission::{AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess, WritableFolders},
  resource::{
    AgentProfile, ModelId, PromptText, ProviderSettings, ResourceBody, ResourceKind, TeamAgentProfile,
    TeamCounterparts, TeamProfileError,
  },
  run::bundle::ExecutionProfile,
  secret::SecretName,
  sharing::{SharedBody, VersionWidening, widens},
  time::Timestamp,
};
use serde_json::{Value, json};

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn connection(provider: AgentProvider) -> Connection {
  Connection::new(
    ConnectionId::generate(),
    name("Laptop"),
    ConnectionKind::Agent(provider),
    AgentEndpoint::Program(std::env::temp_dir().join("agent")),
    Timestamp::from_unix_millis(1),
  )
  .expect("an absolute program fits an agent CLI")
}

fn local_server(id: ConnectionId, title: &str) -> McpServer {
  McpServer::new(
    id,
    McpServerName::new(title).expect("valid server name"),
    McpTransport::Http(HttpServer::new(
      McpUrl::new("https://mcp.example.test/v1").expect("valid URL"),
      McpAuthorization::None,
    )),
    McpEnvironment::default(),
    Timestamp::from_unix_millis(1),
  )
  .expect("valid server")
}

fn secret(value: &str) -> SecretName {
  SecretName::new(value).expect("valid secret name")
}

fn base(cli: AgentProvider) -> TeamAgentProfile {
  let settings = ProviderSettings::new(BTreeMap::from([("reasoning_effort".to_owned(), "high".to_owned())]))
    .expect("valid settings");
  TeamAgentProfile::new(
    name("Builder"),
    "Implements the approved plan.".into(),
    PromptText::new("You implement {{task.title}}.").expect("valid prompt"),
    (cli, ModelId::new("gpt-5.5").expect("valid model"), settings),
  )
}

fn local(connection: ConnectionId, servers: Vec<ConnectionId>) -> AgentProfile {
  AgentProfile::new(
    name("Builder"),
    "Implements the approved plan.".into(),
    PromptText::new("You implement {{task.title}}.").expect("valid prompt"),
    connection,
    ModelId::new("gpt-5.5").expect("valid model"),
    ProviderSettings::default(),
  )
  .with_permissions(AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  ))
  .with_mcp_servers(McpServerSelection::new(servers).expect("distinct servers"))
}

fn text(body: &impl serde::Serialize) -> String {
  serde_json::to_string(body).expect("bodies serialize")
}

#[test]
fn team_agent_profile_names_a_cli_kind_team_ids_and_secret_names() {
  let (docs, build, review, tone) = (
    ResourceId::generate(),
    ResourceId::generate(),
    ResourceId::generate(),
    ResourceId::generate(),
  );
  let profile = base(AgentProvider::CodexCli)
    .with_mcp_servers(vec![docs])
    .and_then(|profile| profile.with_tools(vec![build]))
    .and_then(|profile| profile.with_skills(vec![review]))
    .and_then(|profile| profile.with_instruction_resources(vec![tone]))
    .and_then(|profile| profile.with_secrets(vec![secret("STAGING_TOKEN")]))
    .expect("distinct lists");

  let body: Value = serde_json::to_value(&profile).expect("serializes");

  assert_eq!(body["cli"], "codex_cli");
  assert_eq!(body["mcp_servers"], json!([docs]));
  assert_eq!(body["tools"], json!([build]));
  assert_eq!(body["skills"], json!([review]));
  assert_eq!(body["instruction_resources"], json!([tone]));
  assert_eq!(body["secrets"], json!(["STAGING_TOKEN"]));
  assert_eq!(body["model"], "gpt-5.5");
  assert_eq!(body["settings"], json!({"reasoning_effort": "high"}));
  assert!(body.get("connection").is_none());
  assert_eq!(
    serde_json::from_value::<TeamAgentProfile>(body).expect("reads back"),
    profile
  );
}

#[test]
fn team_agent_profile_lists_left_empty_are_left_out_of_the_body() {
  let body = serde_json::to_value(base(AgentProvider::ClaudeCodeCli)).expect("serializes");
  for list in ["mcp_servers", "tools", "skills", "instruction_resources", "secrets"] {
    assert!(body.get(list).is_none(), "{list}");
  }
  assert_eq!(
    serde_json::from_value::<TeamAgentProfile>(body).expect("reads back"),
    base(AgentProvider::ClaudeCodeCli)
  );
}

#[test]
fn team_agent_profile_refuses_a_body_that_carries_an_id_of_a_computer() {
  let local = local(ConnectionId::generate(), vec![ConnectionId::generate()]);
  let as_local = serde_json::to_value(&local).expect("serializes");
  assert!(
    serde_json::from_value::<TeamAgentProfile>(as_local).is_err(),
    "a local profile's connection has no place in a team profile"
  );

  let team = serde_json::to_value(base(AgentProvider::CodexCli)).expect("serializes");
  for (field, value) in [
    ("connection", json!(ConnectionId::generate())),
    ("secret_ids", json!([ConnectionId::generate()])),
    ("program", json!("/usr/local/bin/mcp")),
  ] {
    let mut body = team.clone();
    body[field] = value;
    assert!(serde_json::from_value::<TeamAgentProfile>(body).is_err(), "{field}");
  }
}

#[test]
fn team_agent_profile_refuses_a_list_that_repeats_an_item_or_is_too_long() {
  let id = ResourceId::generate();
  assert!(matches!(
    base(AgentProvider::CodexCli).with_tools(vec![id, id]),
    Err(TeamProfileError::Duplicate { list: "tools", .. })
  ));
  assert!(matches!(
    base(AgentProvider::CodexCli).with_secrets(vec![secret("TOKEN"), secret("TOKEN")]),
    Err(TeamProfileError::Duplicate { list: "secrets", .. })
  ));
  let many = (0..=TeamAgentProfile::MAX_LISTED)
    .map(|_| ResourceId::generate())
    .collect();
  assert_eq!(
    base(AgentProvider::CodexCli).with_skills(many),
    Err(TeamProfileError::TooMany { list: "skills" })
  );

  let mut body = serde_json::to_value(base(AgentProvider::CodexCli)).expect("serializes");
  body["mcp_servers"] = json!([id, id]);
  assert!(
    serde_json::from_value::<TeamAgentProfile>(body).is_err(),
    "read back through the checks"
  );
}

#[test]
fn team_agent_profile_refuses_a_secret_that_is_not_a_name() {
  let mut body = serde_json::to_value(base(AgentProvider::CodexCli)).expect("serializes");
  body["secrets"] = json!(["staging token"]);
  assert!(serde_json::from_value::<TeamAgentProfile>(body).is_err());
}

#[test]
fn team_agent_profile_is_a_local_profile_with_the_connections_cli_and_the_teams_mcp_servers() {
  let codex = connection(AgentProvider::CodexCli);
  let team_docs = ResourceId::generate();
  let on_docs = ConnectionId::from_uuid(team_docs.as_uuid());
  let local = local(codex.id(), vec![on_docs]);
  let connections = [codex];

  let team = TeamAgentProfile::from_local(
    &local,
    TeamCounterparts {
      connections: &connections,
      local_mcp_servers: &[],
      team_mcp_servers: &[team_docs],
    },
  )
  .expect("a team counterpart for every id");

  assert_eq!(team.cli(), AgentProvider::CodexCli);
  assert_eq!(team.mcp_servers(), [team_docs]);
  assert_eq!(team.role(), local.role());
  assert_eq!(team.model(), local.model());
  assert_eq!(team.permissions(), local.permissions());
  assert_eq!(team.instructions(), local.instructions());
  let sent = text(&team);
  assert!(!sent.contains(&connections[0].id().to_string()), "{sent}");
}

#[test]
fn team_agent_profile_is_refused_for_an_mcp_server_that_is_not_a_team_one() {
  let codex = connection(AgentProvider::CodexCli);
  let own = ConnectionId::generate();
  let local = local(codex.id(), vec![own]);
  let connections = [codex];
  let servers = [local_server(own, "notes")];

  let refused = TeamAgentProfile::from_local(
    &local,
    TeamCounterparts {
      connections: &connections,
      local_mcp_servers: &servers,
      team_mcp_servers: &[ResourceId::generate()],
    },
  );

  assert_eq!(refused, Err(TeamProfileError::LocalMcpServer("notes".to_owned())));
}

#[test]
fn team_agent_profile_is_refused_for_a_connection_that_names_no_cli_kind() {
  let github = Connection::new(
    ConnectionId::generate(),
    name("GitHub"),
    ConnectionKind::GitHub {
      host: GitHubHost::new("github.com").expect("valid host"),
    },
    AgentEndpoint::Program(std::env::temp_dir().join("gh")),
    Timestamp::from_unix_millis(1),
  )
  .expect("a GitHub connection");
  let missing = ConnectionId::generate();
  for chosen in [github.id(), missing] {
    let connections = [github.clone()];
    let refused = TeamAgentProfile::from_local(
      &local(chosen, Vec::new()),
      TeamCounterparts {
        connections: &connections,
        local_mcp_servers: &[],
        team_mcp_servers: &[],
      },
    );
    assert_eq!(refused, Err(TeamProfileError::Connection(chosen)));
  }
}

#[test]
fn team_agent_profile_runs_on_a_connection_of_this_computer_with_the_teams_mcp_servers() {
  let docs = ResourceId::generate();
  let team = base(AgentProvider::CodexCli)
    .with_permissions(AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::AnyInWorktree,
      NetworkAccess::On,
    ))
    .with_mcp_servers(vec![docs])
    .expect("distinct");
  let on = ConnectionId::generate();

  let profile = AgentProfile::from_team(&team, on);

  assert_eq!(profile.connection(), on);
  assert_eq!(profile.mcp_servers().ids(), [ConnectionId::from_uuid(docs.as_uuid())]);
  assert_eq!(profile.role(), team.role());
  assert_eq!(profile.model(), team.model());
  assert_eq!(profile.permissions(), team.permissions());
}

#[test]
fn team_agent_profile_is_a_resource_body_of_the_profile_kind() {
  assert_eq!(<TeamAgentProfile as ResourceBody>::KIND, ResourceKind::AgentProfile);
}

#[test]
fn team_agent_profile_widens_with_permissions_an_mcp_server_or_a_secret_the_version_before_did_not_have() {
  let (docs, notes) = (ResourceId::generate(), ResourceId::generate());
  let v1 = base(AgentProvider::CodexCli)
    .with_mcp_servers(vec![docs])
    .and_then(|profile| profile.with_secrets(vec![secret("TOKEN")]))
    .expect("distinct");

  assert!(widens(Some(&v1), &v1).is_empty(), "the same version widens nothing");

  let more_servers = v1.clone().with_mcp_servers(vec![docs, notes]).expect("distinct");
  assert_eq!(widens(Some(&v1), &more_servers), [VersionWidening::McpServer]);

  let fewer_servers = v1.clone().with_mcp_servers(Vec::new()).expect("distinct");
  assert!(widens(Some(&v1), &fewer_servers).is_empty());

  let more_secrets = v1
    .clone()
    .with_secrets(vec![secret("TOKEN"), secret("OTHER")])
    .expect("distinct");
  assert_eq!(widens(Some(&v1), &more_secrets), [VersionWidening::Secret]);

  let writes = v1.clone().with_permissions(AgentPermissions::new(
    RepositoryAccess::Write,
    CommandPermission::AnyInWorktree,
    NetworkAccess::On,
  ));
  assert!(
    widens(Some(&v1), &writes)
      .iter()
      .all(|widening| matches!(widening, VersionWidening::Permissions(_)))
      && !widens(Some(&v1), &writes).is_empty()
  );
}

#[test]
fn team_agent_profile_first_version_widens_over_nothing_granted() {
  let docs = ResourceId::generate();
  let first = base(AgentProvider::CodexCli)
    .with_mcp_servers(vec![docs])
    .and_then(|profile| profile.with_secrets(vec![secret("TOKEN")]))
    .expect("distinct");

  let widenings = first.widenings_over(None);

  assert!(widenings.contains(&VersionWidening::McpServer));
  assert!(widenings.contains(&VersionWidening::Secret));
  let as_local = AgentProfile::from_team(&base(AgentProvider::CodexCli), ConnectionId::generate());
  assert_eq!(
    base(AgentProvider::CodexCli).widenings_over(None),
    as_local.widenings_over(None),
    "its permissions widen as a local profile's do"
  );
}

#[test]
fn team_agent_profile_pins_the_same_bundle_profile_as_the_local_one_it_came_from() {
  let codex = connection(AgentProvider::CodexCli);
  let local = local(codex.id(), Vec::new());
  let connections = [codex];
  let team = TeamAgentProfile::from_local(
    &local,
    TeamCounterparts {
      connections: &connections,
      local_mcp_servers: &[],
      team_mcp_servers: &[],
    },
  )
  .expect("no MCP server to name");

  assert_eq!(ExecutionProfile::of_team(&team), ExecutionProfile::of(&local));
}

fn with_folders(permissions: AgentPermissions) -> AgentPermissions {
  let folders: WritableFolders = serde_json::from_value(json!(["/opt/cache", "/opt/build"])).expect("folders");
  permissions.with_writable_folders(folders)
}

#[test]
fn team_agent_profile_is_refused_for_a_profile_with_writable_folders() {
  let codex = connection(AgentProvider::CodexCli);
  let local = local(codex.id(), Vec::new());
  let local = local
    .clone()
    .with_permissions(with_folders(local.permissions().clone()));
  let connections = [codex];

  let refused = TeamAgentProfile::from_local(
    &local,
    TeamCounterparts {
      connections: &connections,
      local_mcp_servers: &[],
      team_mcp_servers: &[],
    },
  );

  assert_eq!(
    refused,
    Err(TeamProfileError::WritableFolders {
      folders: vec!["/opt/cache".to_owned(), "/opt/build".to_owned()]
    })
  );
}

#[test]
fn team_agent_profile_refuses_a_body_with_writable_folders() {
  let mut body = serde_json::to_value(base(AgentProvider::CodexCli)).expect("serializes");
  assert!(
    body["permissions"].get("writable_folders").is_none(),
    "none while empty"
  );
  body["permissions"]["writable_folders"] = json!(["/opt/cache"]);

  let refused = serde_json::from_value::<TeamAgentProfile>(body).expect_err("paths of a computer");

  assert!(refused.to_string().contains("/opt/cache"), "{refused}");
}

#[test]
fn team_agent_profile_without_writable_folders_reads_back_and_runs_with_none() {
  let team = base(AgentProvider::CodexCli);
  let sent = serde_json::to_value(&team).expect("serializes");

  let read: TeamAgentProfile = serde_json::from_value(sent).expect("a body without writable folders");

  assert_eq!(read, team);
  assert!(read.permissions().writable_folders().is_empty());
  let on = AgentProfile::from_team(&read, ConnectionId::generate());
  assert!(on.permissions().writable_folders().is_empty());
}
