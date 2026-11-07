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
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, Slug, TeamMcpServer, VersionNumber},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  sharing::{MemberRole, SharedPerson, TaskRevision},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{
  AppDb, CachedVersion, ConnectionPick, ProjectDb, ReferenceTarget, ResourceSharing, SharingLink, StoredServer,
  VersionReference,
};
use orchester_workspace::{tasks::outside::Outside, workspace::Workspace};
use parking_lot::Mutex;
use serde_json::json;
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

/// A team project on a registered team server, with the team's profiles and MCP servers cached, and a local profile.
pub struct Team {
  _dir: TempDir,
  pub root: PathBuf,
  pub app_db: PathBuf,
  pub workspace: Workspace,
  pub server: ServerId,
  pub claude: ConnectionId,
  pub codex: ConnectionId,
  /// A team profile whose newest version names a Claude Code connection.
  pub claude_profile: ResourceId,
  /// A team profile whose newest version names no connection: its CLI kind is not known.
  pub open_profile: ResourceId,
  pub local_profile: ResourceId,
  pub program_server: ResourceId,
  pub remote_server: ResourceId,
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

pub fn cached<B>(local_body: B, references: Vec<VersionReference>) -> CachedVersion<B> {
  CachedVersion {
    number: VersionNumber::FIRST,
    published_at: Timestamp::from_unix_millis(20),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "Olena".to_owned(),
    },
    server_body: json!({}),
    hash: ContentHash::new(&"a".repeat(64)).expect("a hash"),
    references,
    local_body,
    waits_here: false,
  }
}

/// Creates `body` as a team resource at v1 with `references`.
pub fn team_resource<B: orchester_core::resource::ResourceBody>(
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
  db.cache_shared_version(id, &cached(body, references)).expect("cached");
  id
}

pub fn mcp_server(body: serde_json::Value) -> TeamMcpServer {
  serde_json::from_value(body).expect("a team MCP server")
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
    share(&mut db, server);
    let claude = connection(&mut db, &root, AgentProvider::ClaudeCodeCli, "claude");
    let codex = connection(&mut db, &root, AgentProvider::CodexCli, "codex");
    let named = VersionReference {
      target: ReferenceTarget::Connection(ConnectionId::generate()),
      name: "Claude".to_owned(),
      role: Some("claude_code_cli".to_owned()),
    };
    let claude_profile = team_resource(&mut db, "claude-builder", profile(), vec![named]);
    let open_profile = team_resource(&mut db, "open-builder", profile(), Vec::new());
    let program_server = team_resource(
      &mut db,
      "knowledge",
      mcp_server(json!({"name":"knowledge","launch":{"kind":"program","program":"npx"}})),
      Vec::new(),
    );
    let remote_server = team_resource(
      &mut db,
      "issues",
      mcp_server(json!({"name":"issues","launch":{"kind":"remote","url":"https://mcp.example.org/x"}})),
      Vec::new(),
    );
    let local_profile = db
      .create_resource(
        Slug::new("local").expect("slug"),
        Name::new("Local").expect("name"),
        &profile(),
        Timestamp::now(),
      )
      .expect("a local profile")
      .id();
    drop(db);
    Self {
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
    }
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

pub fn pick(connection: ConnectionId) -> ConnectionPick {
  ConnectionPick {
    connection,
    account: None,
  }
}
