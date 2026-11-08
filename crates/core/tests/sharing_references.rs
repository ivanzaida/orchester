//! The connections, MCP servers and secrets a body of this computer names, over explicit bodies of each kind: found
//! (a team body must name none).

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{
  connection::{
    AgentEndpoint, AgentProvider, Connection, ConnectionKind,
    mcp::{
      HttpServer, McpAuthorization, McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, McpUrl,
    },
  },
  id::{ConnectionId, ResourceId, SecretId},
  name::Name,
  permission::StepPermissions,
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, AgentTargets, AllowedSteps, ApprovalMode, DatabaseQuery, DeniedBehavior,
    HttpAuth, HttpMethod, HttpRequest, InjectionEvent, Instruction, InstructionScope, ModelId, Pipeline, PromptText,
    ProviderSettings, RepositoryAccess, Step, StepKey, StepKind, Tool, ToolAction,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName},
  sharing::{ConnectionRole, LocalEntities, Reference, ReferenceKind, named},
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
