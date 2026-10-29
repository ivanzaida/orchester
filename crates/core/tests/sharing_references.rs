//! A shared version's references over explicit bodies of each kind: found, resolved through bindings, and turned back
//! into references when publishing.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  connection::{
    AgentEndpoint, AgentProvider, Connection, ConnectionKind,
    mcp::{
      HttpServer, McpAuthorization, McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, McpUrl,
    },
  },
  id::{ConnectionId, ResourceId, SecretId},
  import::Entity,
  name::Name,
  permission::StepPermissions,
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AgentTargets, AllowedSteps, ApprovalMode, DatabaseQuery, DeniedBehavior,
    HttpAuth, HttpMethod, HttpRequest, InjectionEvent, Instruction, InstructionScope, ModelId, Pipeline, PromptText,
    ProviderSettings, RepositoryAccess, Step, StepKey, StepKind, Tool, ToolAction,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, tool_uses},
  sharing::{
    Binding, BindingKindError, ConnectionRole, LocalEntities, Published, Reference, ReferenceKind, for_publishing,
    named, resolve,
  },
  time::Timestamp,
};
use serde_json::Value;

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn at() -> Timestamp {
  Timestamp::from_unix_millis(1)
}

fn connection(title: &str, provider: AgentProvider) -> Connection {
  Connection::new(
    ConnectionId::generate(),
    name(title),
    ConnectionKind::Agent(provider),
    AgentEndpoint::Program(std::env::temp_dir().join("agent")),
    at(),
  )
  .expect("absolute executable")
}

fn server(title: &str) -> McpServer {
  McpServer::new(
    ConnectionId::generate(),
    McpServerName::new(title).expect("valid server name"),
    McpTransport::Http(HttpServer::new(
      McpUrl::new("https://mcp.example.test/v1").expect("valid URL"),
      McpAuthorization::None,
    )),
    McpEnvironment::default(),
    at(),
  )
  .expect("valid server")
}

fn secret(title: &str, kind: SecretKind) -> Secret {
  Secret::new(
    SecretId::generate(),
    NewSecret {
      name: SecretName::new(title).expect("secret name"),
      kind,
      environment: Environment::Production,
    },
    at(),
  )
}

fn json<T: serde::Serialize>(body: &T) -> Value {
  serde_json::to_value(body).expect("bodies serialize")
}

fn profile(connection: ConnectionId, servers: &[ConnectionId]) -> Value {
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build it.").expect("prompt"),
    connection,
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
  .with_mcp_servers(McpServerSelection::new(servers.to_vec()).expect("servers"));
  json(&profile)
}

/// A tool doing `action`, which a pipeline's every step may call: the policy is keyed by the pipeline's id.
fn tool(action: ToolAction) -> Value {
  let access = AccessPolicy::new(
    BTreeMap::from([(ResourceId::generate(), AllowedSteps::All)]),
    name("Production"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("valid policy");
  json(&Tool::new("", action, Some(access)).expect("valid tool"))
}

fn http_tool(token: SecretId) -> Value {
  let request = HttpRequest::new(
    HttpMethod::Post,
    "https://deploy.example.test/hook".into(),
    "deploy.example.test".into(),
    HttpAuth::Bearer { secret: token },
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .expect("valid request");
  tool(ToolAction::Http(request))
}

fn database_tool(credentials: SecretId) -> Value {
  tool(ToolAction::DatabaseQuery(
    DatabaseQuery::new(credentials, "SELECT 1".into()).expect("valid query"),
  ))
}

fn pipeline(profile: ResourceId, tool: ResourceId) -> Value {
  let step = |key: &str, kind| Step {
    key: StepKey::new(key).expect("key"),
    name: name(key),
    kind,
    instructions: PromptText::new("").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let build = step(
    "build",
    StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
  );
  let deploy = step("deploy", StepKind::Tool { tool: Some(tool) });
  json(&Pipeline::new(vec![build, deploy]).expect("valid pipeline"))
}

fn instruction(pipeline: ResourceId) -> Value {
  let instruction = Instruction::new(
    PromptText::new("Keep the changelog.").expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Pipeline { pipeline },
  )
  .expect("valid instruction");
  json(&instruction)
}

/// A computer's connections, MCP servers and secrets.
struct Computer {
  connections: Vec<Connection>,
  servers: Vec<McpServer>,
  secrets: Vec<Secret>,
}

impl Computer {
  fn entities(&self) -> LocalEntities<'_> {
    LocalEntities {
      connections: &self.connections,
      mcp_servers: &self.servers,
      secrets: &self.secrets,
    }
  }
}

/// The publisher's computer: Claude Code "Work", the GitHub MCP server, an API token and a database's credentials.
fn publisher() -> Computer {
  Computer {
    connections: vec![connection("Work", AgentProvider::ClaudeCodeCli)],
    servers: vec![server("github")],
    secrets: vec![
      secret("API_TOKEN", SecretKind::AccessToken),
      secret("WAREHOUSE", SecretKind::DatabaseConnection),
    ],
  }
}

fn bind(reference: &Reference, local: Entity) -> Binding {
  Binding::new(reference.clone(), local).expect("same kind")
}

#[test]
fn sharing_references_of_a_profile_are_its_connection_and_mcp_servers_by_kind_name_and_role() {
  let computer = publisher();
  let (work, github) = (&computer.connections[0], &computer.servers[0]);
  let body = profile(work.id(), &[github.id()]);

  let references = named(&body, computer.entities());

  assert_eq!(
    references,
    [
      Reference::Connection {
        id: work.id(),
        name: "Work".into(),
        role: ConnectionRole::Agent(AgentProvider::ClaudeCodeCli),
      },
      Reference::McpServer {
        id: github.id(),
        name: "github".into(),
      },
    ]
  );
  let described: Vec<_> = references
    .iter()
    .map(|reference| (reference.kind(), reference.name(), reference.role()))
    .collect();
  assert_eq!(
    described,
    [
      (ReferenceKind::Connection, "Work", Some("claude_code_cli")),
      (ReferenceKind::McpServer, "github", None),
    ]
  );
}

#[test]
fn sharing_references_of_http_and_database_tools_are_their_secrets_with_the_secret_kind() {
  let computer = publisher();
  let (token, warehouse) = (&computer.secrets[0], &computer.secrets[1]);

  let http = named(&http_tool(token.id()), computer.entities());
  let database = named(&database_tool(warehouse.id()), computer.entities());

  assert_eq!(http, [Reference::secret(token)]);
  assert_eq!(http[0].role(), Some("access_token"));
  assert_eq!(database, [Reference::secret(warehouse)]);
  assert_eq!(database[0].role(), Some("database_connection"));
}

#[test]
fn sharing_references_pipelines_and_instructions_name_only_shared_resources() {
  let computer = publisher();
  let (profile, tool, pipeline_id) = (ResourceId::generate(), ResourceId::generate(), ResourceId::generate());

  assert!(named(&pipeline(profile, tool), computer.entities()).is_empty());
  assert!(named(&instruction(pipeline_id), computer.entities()).is_empty());
}

#[test]
fn sharing_references_resolve_bound_references_to_local_ids_and_keep_unbound_ones() {
  let computer = publisher();
  let (work, github) = (&computer.connections[0], &computer.servers[0]);
  let body = profile(work.id(), &[github.id()]);
  let references = named(&body, computer.entities());
  let here = connection("work", AgentProvider::ClaudeCodeCli);

  let resolved = resolve(body, &[bind(&references[0], Entity::Connection(here.id()))]);

  let resolved: AgentProfile = serde_json::from_value(resolved).expect("still a profile");
  assert_eq!(resolved.connection(), here.id());
  assert_eq!(resolved.mcp_servers().ids(), [github.id()]);
}

#[test]
fn sharing_references_resolve_a_tool_secret_and_leave_other_tools_alone() {
  let computer = publisher();
  let (token, warehouse) = (&computer.secrets[0], &computer.secrets[1]);
  let local = secret("API_TOKEN", SecretKind::AccessToken);
  let bindings = [bind(&Reference::secret(token), Entity::Secret(local.id()))];

  let http: Tool = serde_json::from_value(resolve(http_tool(token.id()), &bindings)).expect("still a tool");
  let database: Tool = serde_json::from_value(resolve(database_tool(warehouse.id()), &bindings)).expect("a tool");

  assert!(tool_uses(&http, local.id()));
  assert!(tool_uses(&database, warehouse.id()));
}

#[test]
fn sharing_references_resolve_keeps_shared_resource_ids() {
  let (profile, tool) = (ResourceId::generate(), ResourceId::generate());
  let body = pipeline(profile, tool);
  let computer = publisher();
  let bindings = [bind(
    &Reference::connection(&computer.connections[0]),
    Entity::Connection(ConnectionId::generate()),
  )];

  assert_eq!(resolve(body.clone(), &bindings), body);
}

#[test]
fn sharing_references_publishing_turns_bound_local_ids_back_into_their_references() {
  let publisher = publisher();
  let (work, github) = (&publisher.connections[0], &publisher.servers[0]);
  let published = profile(work.id(), &[github.id()]);
  let references = named(&published, publisher.entities());
  let here = Computer {
    connections: vec![connection("My Claude", AgentProvider::ClaudeCodeCli)],
    servers: vec![server("github-mine")],
    secrets: Vec::new(),
  };
  let bindings = [
    bind(&references[0], Entity::Connection(here.connections[0].id())),
    bind(&references[1], Entity::McpServer(here.servers[0].id())),
  ];
  let local = resolve(published.clone(), &bindings);

  let again = for_publishing(local, here.entities(), &references, &bindings);

  assert_eq!(
    again,
    Published {
      body: published,
      references,
    }
  );
}

#[test]
fn sharing_references_publishing_names_new_local_entities_by_kind_name_and_role_only() {
  let publisher = publisher();
  let work = &publisher.connections[0];
  let base_body = profile(work.id(), &[]);
  let base = named(&base_body, publisher.entities());
  let here = Computer {
    connections: vec![connection("Codex", AgentProvider::CodexCli)],
    servers: vec![server("docs")],
    secrets: vec![secret("DEPLOY_KEY", SecretKind::DeploymentToken)],
  };
  let edited = profile(here.connections[0].id(), &[here.servers[0].id()]);

  let again = for_publishing(edited.clone(), here.entities(), &base, &[]);

  assert_eq!(again.body, edited);
  let described: Vec<_> = again
    .references
    .iter()
    .map(|reference| (reference.entity(), reference.kind(), reference.name(), reference.role()))
    .collect();
  assert_eq!(
    described,
    [
      (
        Entity::Connection(here.connections[0].id()),
        ReferenceKind::Connection,
        "Codex",
        Some("codex_cli")
      ),
      (
        Entity::McpServer(here.servers[0].id()),
        ReferenceKind::McpServer,
        "docs",
        None
      ),
    ]
  );
  let token = for_publishing(http_tool(here.secrets[0].id()), here.entities(), &[], &[]);
  assert_eq!(
    token.references,
    [Reference::Secret {
      id: here.secrets[0].id(),
      name: "DEPLOY_KEY".into(),
      role: SecretKind::DeploymentToken,
    }]
  );
}

#[test]
fn sharing_references_publishing_keeps_an_unbound_reference_of_the_base_version() {
  let publisher = publisher();
  let github = &publisher.servers[0];
  let here = Computer {
    connections: vec![connection("Claude", AgentProvider::ClaudeCodeCli)],
    servers: Vec::new(),
    secrets: Vec::new(),
  };
  let base = named(
    &profile(publisher.connections[0].id(), &[github.id()]),
    publisher.entities(),
  );
  let bindings = [bind(&base[0], Entity::Connection(here.connections[0].id()))];
  let local = resolve(profile(publisher.connections[0].id(), &[github.id()]), &bindings);

  let again = for_publishing(local, here.entities(), &base, &bindings);

  assert_eq!(again.references, base);
}

#[test]
fn sharing_references_a_binding_is_of_the_reference_kind() {
  let computer = publisher();
  let reference = Reference::mcp_server(&computer.servers[0]);

  let error: BindingKindError =
    Binding::new(reference.clone(), Entity::Connection(ConnectionId::generate())).expect_err("another kind");
  assert_eq!(
    error.to_string(),
    "a mcp_server reference is bound only to a local entity of that kind"
  );
  assert!(Binding::new(reference.clone(), Entity::Resource(ResourceId::generate())).is_err());
  assert!(Binding::new(reference, Entity::McpServer(ConnectionId::generate())).is_ok());
}
