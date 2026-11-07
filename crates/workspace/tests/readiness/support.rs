//! The projects the tests read: a team project with team resources cached the way the stream keeps them, and a local
//! one.

use std::{
  collections::{BTreeSet, HashMap},
  fs,
  path::PathBuf,
};

use orchester_core::{
  blob::ContentHash,
  connection::{AgentConnection, AgentProvider},
  id::{ConnectionId, ProjectId, ResourceId, ServerId, UserId},
  name::Name,
  permission::StepPermissions,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, ResourceBody, Slug, Step, StepKey,
    StepKind, TeamAgentProfile, TeamMcpServer, VersionNumber,
  },
  secret::{SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  sharing::{MemberRole, SharedPerson, TaskRevision},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{AppDb, CachedVersion, ProjectDb, ResourceSharing, SharingLink, StoredServer};
use orchester_workspace::{readiness::ProjectReadiness, workspace::Workspace};
use parking_lot::Mutex;
use serde::Serialize;
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

pub fn name(text: &str) -> Name {
  Name::new(text).expect("valid name")
}

pub fn secret(text: &str) -> SecretName {
  SecretName::new(text).expect("valid secret name")
}

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

pub struct Fixture {
  _dir: TempDir,
  pub root: PathBuf,
  pub workspace: Workspace,
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
    Self {
      _dir: dir,
      root,
      workspace: Workspace::new(app),
    }
  }

  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.root).expect("project database")
  }

  pub fn read(&self) -> ProjectReadiness {
    self.workspace.project_readiness(&self.root).expect("readiness")
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

  /// The team's profile at v1: the server holds `team`, this computer runs it by a local body.
  pub fn team_profile(&self, slug: &str, team: &TeamAgentProfile) -> ResourceId {
    let mut db = self.db();
    let id = self.create(&mut db, slug, &AgentProfile::from_team(team, ConnectionId::generate()));
    let local = AgentProfile::from_team(team, AgentProfile::unresolved_connection(id));
    cache(
      &mut db,
      id,
      (VersionNumber::FIRST, false),
      serde_json::to_value(team).expect("json"),
      local,
    );
    id
  }

  /// The team's resource of any kind whose body is the same on the server and here, at v1.
  pub fn team_resource<B: ResourceBody + Serialize + Clone>(&self, slug: &str, body: B) -> ResourceId {
    let mut db = self.db();
    let id = self.create(&mut db, slug, &body);
    cache(
      &mut db,
      id,
      (VersionNumber::FIRST, false),
      serde_json::to_value(&body).expect("json"),
      body,
    );
    id
  }

  /// A team MCP server from its body as the server sends it.
  pub fn team_server(&self, slug: &str, body: Value) -> ResourceId {
    self.team_resource(slug, server_body(body))
  }

  /// A resource of this computer, published at v1: a local project's own, which no server knows.
  pub fn own<B: ResourceBody>(&self, slug: &str, body: &B) -> ResourceId {
    let mut db = self.db();
    let id = self.create(&mut db, slug, body);
    db.publish_draft::<B>(id, at(11)).expect("published");
    id
  }

  fn create<B: ResourceBody>(&self, db: &mut ProjectDb, slug: &str, body: &B) -> ResourceId {
    let id = db
      .create_resource(Slug::new(slug).expect("slug"), name(slug), body, at(10))
      .expect("resource")
      .id();
    if db.sharing().expect("read").is_some() {
      db.set_resource_sharing(
        id,
        Some(ResourceSharing::Shared(TaskRevision::try_from(1).expect("revision"))),
      )
      .expect("shared");
    }
    id
  }

  /// A newer version of the team's `profile`; `waits` is a teammate's version that waits for *Use v<N> here*.
  pub fn next_profile_version(&self, profile: ResourceId, number: VersionNumber, team: &TeamAgentProfile, waits: bool) {
    let local = AgentProfile::from_team(team, AgentProfile::unresolved_connection(profile));
    let server_body = serde_json::to_value(team).expect("json");
    cache(&mut self.db(), profile, (number, waits), server_body, local);
  }
}

fn cache<B: ResourceBody>(
  db: &mut ProjectDb,
  id: ResourceId,
  (number, waits_here): (VersionNumber, bool),
  server_body: Value,
  local_body: B,
) {
  let cached = CachedVersion {
    number,
    published_at: at(20),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Olena".to_owned(),
    },
    server_body,
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references: Vec::new(),
    local_body,
    waits_here,
  };
  db.cache_shared_version(id, &cached).expect("cached");
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
