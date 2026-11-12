//! The team project the tests fill in, and the keychains they use.

use std::{
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
};

use orchester_core::{
  blob::ContentHash,
  connection::{AgentConnection, AgentProvider},
  id::{ConnectionId, ProjectId, ResourceId, ServerId, UserId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{
    AgentProfile, ModelId, PromptText, ProviderSettings, ResourceBody, ResourceKind, Slug, TeamAgentProfile,
    VersionNumber,
  },
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::{MemberRole, SharedPerson, TaskRevision},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{AppDb, CachedVersion, ConnectionPick, ProjectDb, ResourceSharing, SharingLink, StoredServer};
use orchester_workspace::{
  agents::team_hub::{TeamHub, TeamHubRead},
  tasks::outside::Outside,
  team_config::{TeamConfig, TeamResource, TeamVersion},
  workspace::Workspace,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tempfile::TempDir;

pub const FAKE: &str = "fake-value-7Qx2";

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

/// A keychain that refuses, as a locked one does.
pub struct LockedVault;

impl SecretVault for LockedVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::AccessDenied("the keychain is locked".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

/// A team project on a registered team server, with the team's profiles and MCP servers kept in the workspace's memory
/// as a read of the team server keeps them, and a local profile.
pub struct Team {
  _dir: TempDir,
  pub root: PathBuf,
  pub app_db: PathBuf,
  pub workspace: Workspace,
  pub server: ServerId,
  pub claude: ConnectionId,
  pub codex: ConnectionId,
  /// A team profile whose latest version names the Claude Code CLI kind.
  pub claude_profile: ResourceId,
  /// A team profile whose latest version is not in the team's form: its CLI kind is not known.
  pub open_profile: ResourceId,
  pub local_profile: ResourceId,
  pub program_server: ResourceId,
  pub remote_server: ResourceId,
  /// The team's resources as its server answers them.
  team: Mutex<Vec<TeamResource>>,
}

pub fn name(text: &str) -> Name {
  Name::new(text).expect("valid name")
}

pub fn profile() -> AgentProfile {
  AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("model"),
    ProviderSettings::default(),
  )
}

/// A team profile Builder that runs on `cli`.
pub fn team_profile(cli: AgentProvider) -> TeamAgentProfile {
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
}

/// The team's resource `slug` whose latest version v1 is `body` as the server sent it.
fn team_resource_of(id: ResourceId, slug: &str, kind: ResourceKind, body: Value) -> TeamResource {
  TeamResource {
    id,
    kind,
    name: name(slug),
    archived: false,
    revision: TaskRevision::try_from(1).expect("revision"),
    updated: orchester_workspace::team_config::Changed {
      by: "Olena".to_owned(),
      at: orchester_core::time::Timestamp::from_unix_millis(0),
    },
    latest: Some(TeamVersion {
      number: VersionNumber::FIRST,
      body,
      published_by: "Olena".to_owned(),
      published_at: Timestamp::from_unix_millis(20),
    }),
  }
}

/// Caches `team` in the project's database at v1 as the stream caches a team profile, with a local body that names no
/// connection of this computer: for what reads and refreshes the cache's local body.
fn cache_profile(db: &mut ProjectDb, slug: &str, team: &TeamAgentProfile) -> ResourceId {
  let local = AgentProfile::from_team(team, ConnectionId::generate());
  let id = db
    .create_resource(
      Slug::new(slug).expect("slug"),
      name(slug),
      &local,
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
    server_body: serde_json::to_value(team).expect("a team profile serializes"),
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references: Vec::new(),
    local_body: local,
  };
  db.cache_shared_version(id, &version).expect("the team form cached");
  id
}

pub fn connection(db: &mut ProjectDb, root: &Path, provider: AgentProvider, label: &str) -> ConnectionId {
  let made = AgentConnection::new(
    ConnectionId::generate(),
    name(label),
    provider,
    root.join(format!("{label}.exe")),
    Timestamp::now(),
  )
  .expect("connection");
  db.create_connection(&made).expect("a connection");
  made.id()
}

/// Shares the project on `server`.
pub fn share(db: &mut ProjectDb, server: ServerId) {
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
}

/// The team's two profiles (one in the team's form, one not) and two MCP servers (a program and a remote one).
fn team_resources() -> [TeamResource; 4] {
  let claude = serde_json::to_value(team_profile(AgentProvider::ClaudeCodeCli)).expect("json");
  let open = serde_json::to_value(profile()).expect("json");
  let program = json!({"name": "knowledge", "launch": {"kind": "program", "program": "npx"}});
  let remote = json!({"name": "issues", "launch": {"kind": "remote", "url": "https://mcp.example.org/x"}});
  [
    ("claude-builder", ResourceKind::AgentProfile, claude),
    ("open-builder", ResourceKind::AgentProfile, open),
    ("knowledge", ResourceKind::McpServer, program),
    ("issues", ResourceKind::McpServer, remote),
  ]
  .map(|(slug, kind, body)| team_resource_of(ResourceId::generate(), slug, kind, body))
}

impl Team {
  pub fn new() -> Self {
    let dir = tempfile::tempdir().expect("temporary directory");
    let root = dir.path().join("repo");
    fs::create_dir_all(root.join(".git")).expect("repository");
    let app_db = dir.path().join("orchester.db");
    let mut app = AppDb::open(&app_db).expect("application database");
    let server = ServerId::generate();
    app
      .add_server(
        &StoredServer {
          id: server,
          url: "https://team.test".to_owned(),
          name: "Team".to_owned(),
          plain_http_allowed: false,
          certificate_pin: None,
          added_at: Timestamp::from_unix_millis(1),
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
    // Made before the project is the team's: a team project makes no resource of its kind here.
    let local_profile = db
      .create_resource(
        Slug::new("local").expect("slug"),
        Name::new("Local").expect("name"),
        &profile(),
        Timestamp::now(),
      )
      .expect("a local profile")
      .id();
    share(&mut db, server);
    let claude = connection(&mut db, &root, AgentProvider::ClaudeCodeCli, "claude");
    let codex = connection(&mut db, &root, AgentProvider::CodexCli, "codex");
    drop(db);
    let kept = team_resources();
    let [claude_profile, open_profile, program_server, remote_server] = kept.each_ref().map(|resource| resource.id);
    let team = Self {
      workspace: Workspace::new(app),
      _dir: dir,
      root,
      app_db,
      server,
      claude,
      codex,
      claude_profile,
      open_profile,
      local_profile,
      program_server,
      remote_server,
      team: Mutex::new(kept.into()),
    };
    team.keep_team();
    team
  }

  /// Keeps the team's resources in the workspace's memory, as a read of the team server does.
  fn keep_team(&self) {
    let config = TeamConfig::new(self.team.lock().iter().cloned());
    self.workspace.keep_team_config_for_test(&self.root, config);
  }

  fn add(&self, resource: TeamResource) {
    self.team.lock().push(resource);
    self.keep_team();
  }

  /// The team's resource `slug` of `kind` whose latest version v1 is `body` as the server sent it, which may be in no
  /// form this computer reads.
  pub fn keep(&self, slug: &str, kind: ResourceKind, body: Value) -> ResourceId {
    let resource = team_resource_of(ResourceId::generate(), slug, kind, body);
    let id = resource.id;
    self.add(resource);
    id
  }

  /// The team's resource `slug` whose latest version v1 is `body`, in its kind's team form.
  pub fn team_resource<B: ResourceBody>(&self, slug: &str, body: &B) -> ResourceId {
    self.keep(slug, B::KIND, serde_json::to_value(body).expect("json"))
  }

  /// The team's profile `slug` at v1, as [`Self::team_resource`] keeps it and also cached in the project's database,
  /// whose local body a fill-in refreshes.
  pub fn cached_profile(&self, slug: &str, team: &TeamAgentProfile) -> ResourceId {
    let id = cache_profile(&mut self.db(), slug, team);
    let body = serde_json::to_value(team).expect("json");
    self.add(team_resource_of(id, slug, ResourceKind::AgentProfile, body));
    id
  }

  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.root).expect("project database")
  }

  /// How often the connections of the project at `root` were counted as changed by a fill-in, on which the screens read
  /// them again.
  pub fn changes_of(&self, root: &Path) -> u64 {
    let project = ProjectDb::open(root).expect("project database").project().id();
    self
      .workspace
      .outside_changes()
      .counts(project)
      .of(Outside::Connections)
  }

  /// [`Self::changes_of`] the team's own project.
  pub fn connection_changes(&self) -> u64 {
    self.changes_of(&self.root)
  }

  /// Another project of this computer, shared on `server`, opened in the workspace.
  pub fn add_project(&self, folder: &str, key: &str, server: ServerId) -> PathBuf {
    let root = self._dir.path().join(folder);
    fs::create_dir_all(root.join(".git")).expect("repository");
    let project = Project::new(ProjectId::generate(), ProjectKey::new(key).expect("key"), name(folder));
    let mut db = ProjectDb::create(&root, project).expect("create project");
    share(&mut db, server);
    drop(db);
    self.workspace.open(&root).expect("opened");
    root
  }

  pub fn program_set(&self, server: ResourceId) -> Option<PathBuf> {
    AppDb::open(&self.app_db)
      .expect("application database")
      .mcp_server_program(self.server, server)
      .expect("read")
      .map(|stored| stored.program)
  }

  /// A file a person could choose as a program.
  pub fn program_file(&self) -> PathBuf {
    let path = self.root.join("mcp-server");
    fs::write(&path, "#!/bin/sh\n").expect("a program");
    path
  }
}

/// The hub of the team's project, whose configuration the tests keep in memory.
pub fn hub(team: &Team, vault: &MemoryVault) -> TeamHub {
  match team.workspace.team_hub(&team.root, vault).expect("hub") {
    TeamHubRead::Read(hub) => *hub,
    other => panic!("the team's configuration is kept in memory, yet the hub read {other:?}"),
  }
}

pub fn pick(connection: ConnectionId) -> ConnectionPick {
  ConnectionPick {
    connection,
    account: None,
  }
}
