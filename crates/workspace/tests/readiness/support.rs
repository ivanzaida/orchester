//! The projects the tests read: a team project with team resources cached the way the stream keeps them, and a local
//! one.

use std::{
  collections::{BTreeMap, BTreeSet, HashMap},
  fs,
  path::PathBuf,
};

use orchester_core::{
  connection::{
    AgentConnection, AgentProvider,
    mcp::{McpEnvironment, McpServer, McpServerName, McpServerSelection, McpTransport, StdioServer},
  },
  id::{ConnectionId, ProjectId, ResourceId, SecretId, ServerId},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AccessPolicy, AccessWindow, AgentProfile, ApprovalMode, CommandEnvironment, CommandProgram, DeniedBehavior,
    HttpAuth, HttpMethod, HttpRequest, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, ResourceBody,
    ResourceKind, Slug, Step, StepKey, StepKind, TeamAgentProfile, TeamMcpServer, Tool, ToolAction, VariableName,
    VersionNumber,
  },
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  sharing::{MemberRole, TaskRevision},
  store::{ConnectionStore, McpServerStore, ResourceStore, SecretStore, SettingsStore},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb, SharingLink, StoredServer};
use orchester_workspace::{
  readiness::ProjectReadiness,
  team_config::{TeamConfig, TeamResource, TeamVersion},
  workspace::Workspace,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tempfile::TempDir;

#[derive(Default)]
pub struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::NotFound)
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

/// A keychain that is locked: it cannot say whether it has any value.
pub struct LockedVault;

impl SecretVault for LockedVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::AccessDenied("locked".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::AccessDenied("locked".to_owned()))
  }

  fn exists(&self, _: &VaultKey) -> Result<bool, VaultError> {
    Err(VaultError::AccessDenied("locked".to_owned()))
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Err(VaultError::AccessDenied("locked".to_owned()))
  }
}

pub fn name(text: &str) -> Name {
  Name::new(text).expect("valid name")
}

pub fn secret(text: &str) -> SecretName {
  SecretName::new(text).expect("valid secret name")
}

pub fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

pub struct Fixture {
  dir: TempDir,
  pub root: PathBuf,
  pub workspace: Workspace,
  pub vault: MemoryVault,
  /// The team's resources as its server would answer them, kept in the workspace's memory.
  team: Mutex<Vec<TeamResource>>,
}

impl Fixture {
  /// A project shared with a team server registered on this computer.
  pub fn team() -> Self {
    Self::new(true)
  }

  pub fn local() -> Self {
    Self::new(false)
  }

  fn new(shared: bool) -> Self {
    let dir = tempfile::tempdir().expect("temporary directory");
    let root = dir.path().join("repo");
    fs::create_dir_all(root.join(".git")).expect("repository");
    let mut app = AppDb::open(&dir.path().join("orchester.db")).expect("application database");
    let server = ServerId::generate();
    app
      .add_server(
        &StoredServer {
          id: server,
          url: "https://team.test".to_owned(),
          name: "Team".to_owned(),
          plain_http_allowed: false,
          certificate_pin: None,
          added_at: at(1),
          joined_with: None,
          account: None,
        },
        None,
      )
      .expect("server");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      name("Kontur"),
    );
    let mut db = ProjectDb::create(&root, project).expect("create project");
    if shared {
      db.start_sharing(
        &SharingLink {
          server,
          project: ProjectId::generate(),
          key: ProjectKey::new("KON").expect("key"),
          role: Some(MemberRole::Owner),
          joined_at: at(100),
          cursor: 0,
          last_synced_at: at(100),
          removed_at: None,
          address: None,
        },
        &[],
        (&[], &HashMap::new()),
      )
      .expect("shared");
    }
    drop(db);
    let fixture = Self {
      dir,
      root,
      workspace: Workspace::new(app),
      vault: MemoryVault::default(),
      team: Mutex::new(Vec::new()),
    };
    if shared {
      fixture.keep_team();
    }
    fixture
  }

  /// Keeps the team's resources in the workspace's memory, as a read of the team server does.
  fn keep_team(&self) {
    let config = TeamConfig::new(self.team.lock().iter().cloned());
    self.workspace.keep_team_config_for_test(&self.root, config);
  }

  /// Keeps `id` with `body` as its latest version `number`, as the team server holds it.
  fn keep_version(&self, id: ResourceId, kind: ResourceKind, slug: &str, (number, body): (VersionNumber, Value)) {
    let latest = TeamVersion {
      number,
      body,
      published_by: "Olena".to_owned(),
      published_at: at(20),
    };
    let mut team = self.team.lock();
    match team.iter_mut().find(|resource| resource.id == id) {
      Some(resource) => resource.latest = Some(latest),
      None => team.push(TeamResource {
        id,
        kind,
        name: name(slug),
        archived: false,
        revision: TaskRevision::try_from(1).expect("revision"),
        updated: orchester_workspace::team_config::Changed {
          by: "Alice Moreau".to_owned(),
          at: orchester_core::time::Timestamp::from_unix_millis(0),
        },
        latest: Some(latest),
      }),
    }
    drop(team);
    self.keep_team();
  }

  /// The team archives `id`, or restores it.
  pub fn archive(&self, id: ResourceId, archived: bool) {
    let mut team = self.team.lock();
    let resource = team
      .iter_mut()
      .find(|resource| resource.id == id)
      .expect("a team resource");
    resource.archived = archived;
    drop(team);
    self.keep_team();
  }

  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.root).expect("project database")
  }

  pub fn read(&self) -> ProjectReadiness {
    self
      .workspace
      .project_readiness(&self.root, &self.vault)
      .expect("readiness")
  }

  pub fn connection(&self, provider: AgentProvider, label: &str) -> ConnectionId {
    let made = AgentConnection::new(
      ConnectionId::generate(),
      name(label),
      provider,
      self.root.join(format!("{label}.exe")),
      Timestamp::now(),
    )
    .expect("connection");
    self.db().create_connection(&made).expect("a connection");
    made.id()
  }

  /// A file a person could choose as a program.
  pub fn program_file(&self, file: &str) -> PathBuf {
    let path = self.root.join(file);
    fs::write(&path, "#!/bin/sh\n").expect("a program");
    path
  }

  /// The folder that holds the project's repository, outside it.
  pub fn dir_path(&self) -> &std::path::Path {
    self.dir.path()
  }

  /// A file a person could choose as a program, outside the repository as a run requires of a fixed file.
  pub fn program_outside(&self, file: &str) -> PathBuf {
    let path = self.dir.path().join(file);
    fs::write(&path, "#!/bin/sh\n").expect("a program");
    path
  }

  /// The team's profile at v1, as the server holds it.
  pub fn team_profile(&self, slug: &str, team: &TeamAgentProfile) -> ResourceId {
    let id = ResourceId::generate();
    let body = serde_json::to_value(team).expect("json");
    self.keep_version(id, ResourceKind::AgentProfile, slug, (VersionNumber::FIRST, body));
    id
  }

  /// The team's resource of any kind at v1, as the server holds it.
  pub fn team_resource<B: ResourceBody>(&self, slug: &str, body: &B) -> ResourceId {
    let id = ResourceId::generate();
    let value = serde_json::to_value(body).expect("json");
    self.keep_version(id, B::KIND, slug, (VersionNumber::FIRST, value));
    id
  }

  /// The team's resource of `kind` whose latest version is `body` as the server sent it, which may be in no form this
  /// computer reads.
  pub fn team_raw(&self, slug: &str, kind: ResourceKind, body: Value) -> ResourceId {
    let id = ResourceId::generate();
    self.keep_version(id, kind, slug, (VersionNumber::FIRST, body));
    id
  }

  /// A team MCP server from its body as the server sends it.
  pub fn team_server(&self, slug: &str, body: Value) -> ResourceId {
    self.team_resource(slug, &server_body(body))
  }

  /// A resource of this computer, published at v1: a local project's own, which no server knows.
  pub fn own<B: ResourceBody>(&self, slug: &str, body: &B) -> ResourceId {
    let mut db = self.db();
    let id = self.create(&mut db, slug, body);
    db.publish_draft::<B>(id, at(11)).expect("published");
    id
  }

  fn create<B: ResourceBody>(&self, db: &mut ProjectDb, slug: &str, body: &B) -> ResourceId {
    db.create_resource(Slug::new(slug).expect("slug"), name(slug), body, at(10))
      .expect("resource")
      .id()
  }

  /// A server of this computer that starts `program`.
  pub fn local_server(&self, label: &str, program: &str) -> ConnectionId {
    let server = McpServer::new(
      ConnectionId::generate(),
      McpServerName::new(label).expect("server name"),
      McpTransport::Stdio(StdioServer::new(
        CommandProgram::new(program).expect("program"),
        vec![],
        None,
      )),
      McpEnvironment::default(),
      at(5),
    )
    .expect("server");
    self.db().create_mcp_server(&server).expect("stored");
    server.id()
  }

  /// Turns the server `id` off in Connections.
  pub fn disable_server(&self, id: ConnectionId) {
    let mut db = self.db();
    let mut server = db.mcp_server(id).expect("read").expect("the server");
    server.set_enabled(false);
    db.save_mcp_server(&server).expect("saved");
  }

  /// The project's Environment sets `PATH` to `folder`.
  pub fn project_path(&self, folder: &std::path::Path) {
    let name = VariableName::new("PATH").expect("variable name");
    let environment = CommandEnvironment::new(BTreeMap::from([(name, folder.display().to_string())])).expect("env");
    self.db().save_project_environment(&environment).expect("saved");
  }

  /// A secret of the project; its value is in the keychain when `value` is.
  pub fn project_secret(&self, label: &str, value: Option<&str>) -> SecretId {
    let mut db = self.db();
    let secret = Secret::new(
      SecretId::generate(),
      NewSecret {
        name: secret(label),
        kind: SecretKind::AccessToken,
        environment: Environment::Production,
      },
      at(6),
    );
    db.create_secret(&secret).expect("stored");
    if let Some(value) = value {
      let value = SecretValue::new(value.to_owned()).expect("value");
      self
        .vault
        .store(&secret.vault_key(db.project().id()), &value)
        .expect("in the keychain");
    }
    secret.id()
  }

  /// The team publishes `team` as `number`, the latest version of its `profile`.
  pub fn next_profile_version(&self, profile: ResourceId, number: VersionNumber, team: &TeamAgentProfile) {
    let body = serde_json::to_value(team).expect("json");
    self.keep_version(profile, ResourceKind::AgentProfile, "", (number, body));
  }
}

/// A tool that sends `secret` as the bearer token of its request.
pub fn http_tool(secret: SecretId) -> Tool {
  let request = HttpRequest::new(
    HttpMethod::Post,
    "https://deploy.example.test/hook".into(),
    "deploy.example.test".into(),
    HttpAuth::Bearer { secret },
    BTreeMap::new(),
    BTreeMap::new(),
  )
  .expect("a request");
  let access = AccessPolicy::new(
    BTreeMap::from([(ResourceId::generate(), orchester_core::resource::AllowedSteps::All)]),
    name("Production"),
    ApprovalMode::EveryCall,
    AccessWindow::UntilStepEnds,
    DeniedBehavior::AwaitChangedPlan,
  )
  .expect("a policy");
  Tool::new("", ToolAction::Http(request), Some(access)).expect("a tool")
}

/// A local profile of `connection` that chooses `servers`.
pub fn profile_on(connection: ConnectionId, servers: &[ConnectionId]) -> AgentProfile {
  AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    connection,
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
  .with_mcp_servers(McpServerSelection::new(servers.to_vec()).expect("distinct servers"))
}

pub fn server_body(body: Value) -> TeamMcpServer {
  serde_json::from_value(body).expect("a team MCP server")
}

/// A program server named `knowledge` whose variables take the team secrets of the same names.
pub fn program(variables: &[&str]) -> Value {
  json!({"name": "knowledge", "launch": {"kind": "program", "program": "npx", "variables": variables}})
}

/// A remote server whose one secret header takes `secret`.
pub fn remote(secret: &str) -> Value {
  json!({"name": "issues", "launch": {
    "kind": "remote", "url": "https://mcp.example.org/x", "secret_headers": [{"header": "X-Key", "secret": secret}]
  }})
}

/// A remote server with two secret headers, which an agent's CLI cannot be given here.
pub fn remote_two_headers(first: &str, second: &str) -> Value {
  json!({"name": "issues", "launch": {
    "kind": "remote", "url": "https://mcp.example.org/x",
    "secret_headers": [{"header": "X-Key", "secret": first}, {"header": "X-Org", "secret": second}]
  }})
}

pub fn profile(cli: AgentProvider, servers: &[ResourceId], secrets: &[&str]) -> TeamAgentProfile {
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
  .with_secrets(secrets.iter().map(|text| secret(text)).collect())
  .expect("distinct secrets")
}

/// A team instruction every agent gets.
pub fn instruction() -> orchester_core::resource::Instruction {
  use orchester_core::resource::{AgentTargets, InjectionEvent, Instruction, InstructionScope};
  Instruction::new(
    PromptText::new("Follow the standards.").expect("prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("an instruction")
}

/// A team skill.
pub fn skill() -> orchester_core::resource::Skill {
  use orchester_core::resource::{InstructionScope, Skill};
  Skill::new(
    "Review a diff.".into(),
    "# Review".into(),
    Vec::new(),
    InstructionScope::Project,
  )
  .expect("skill")
}

/// A pipeline whose agent steps run `profiles` in turn, merged with `integrator`.
pub fn pipeline(profiles: &[Option<ResourceId>], integrator: Option<ResourceId>) -> Pipeline {
  use orchester_core::resource::{DeliverMode, DeliverStep, MergeMethod};
  let steps = profiles
    .iter()
    .enumerate()
    .map(|(at, profile)| Step {
      key: StepKey::new(&format!("step-{at}")).expect("valid key"),
      name: name(&format!("Step {at}")),
      kind: StepKind::Agent {
        profile: *profile,
        repository: RepositoryAccess::ReadOnly,
        model: None,
        permissions: StepPermissions::default(),
      },
      instructions: PromptText::new("").expect("valid prompt"),
      depends_on: BTreeSet::new(),
      approval_gate: false,
      on_failure: None,
      budget: None,
    })
    .collect();
  let deliver = integrator.map(|id| DeliverStep::new(DeliverMode::AutoMerge, MergeMethod::Squash, Some(id), true));
  Pipeline::new(steps).expect("a valid pipeline").with_deliver(deliver)
}
