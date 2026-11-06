//! What this computer fills in for the team's configuration: the connection a team profile runs on, the program of a
//! team MCP server, secret values by name, and which team MCP servers are on here. Each write checks that it names
//! the team's, turning on and setting a program take a person, and no secret value reaches an error or `Debug`.
//!
//! `cargo test -p orchester-workspace --test team_fill_ins team_fill_ins`

use std::{collections::HashMap, fs, path::PathBuf};

use orchester_core::{
  account::AccountCli,
  blob::ContentHash,
  connection::{AgentConnection, AgentProvider},
  id::{ConnectionId, ProjectId, ResourceId, ServerId, UserId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{AgentProfile, ModelId, PromptText, ProviderSettings, Slug, TeamMcpServer, VersionNumber},
  run::Answerer,
  secret::{SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  sharing::{MemberRole, SharedPerson, TaskRevision},
  store::{ConnectionStore, ResourceStore},
  time::Timestamp,
};
use orchester_store::{
  AppDb, CachedVersion, ConnectionPick, ProjectDb, ReferenceTarget, ResourceSharing, SharingLink, StoredServer,
  VersionReference,
};
use orchester_workspace::{tasks::outside::Outside, team_fill_ins::FillInError, workspace::Workspace};
use parking_lot::Mutex;
use serde_json::json;
use tempfile::TempDir;

const FAKE: &str = "fake-value-7Qx2";

#[derive(Default)]
struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

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
struct LockedVault;

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
struct Team {
  _dir: TempDir,
  root: PathBuf,
  app_db: PathBuf,
  workspace: Workspace,
  server: ServerId,
  claude: ConnectionId,
  codex: ConnectionId,
  /// A team profile whose newest version names a Claude Code connection.
  claude_profile: ResourceId,
  /// A team profile whose newest version names no connection: its CLI kind is not known.
  open_profile: ResourceId,
  local_profile: ResourceId,
  program_server: ResourceId,
  remote_server: ResourceId,
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

fn cached<B>(local_body: B, references: Vec<VersionReference>) -> CachedVersion<B> {
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
fn team_resource<B: orchester_core::resource::ResourceBody>(
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

fn mcp_server(body: serde_json::Value) -> TeamMcpServer {
  serde_json::from_value(body).expect("a team MCP server")
}

fn connection(db: &mut ProjectDb, root: &std::path::Path, provider: AgentProvider, label: &str) -> ConnectionId {
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
fn share(db: &mut ProjectDb, server: ServerId) {
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
  fn new() -> Self {
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

  fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.root).expect("project database")
  }

  /// How often the project's connections changed outside the screens, which the screens read again on.
  fn connection_changes(&self) -> u64 {
    let project = self.db().project().id();
    self
      .workspace
      .outside_changes()
      .counts(project)
      .of(Outside::Connections)
  }

  fn program_set(&self, server: ResourceId) -> Option<PathBuf> {
    AppDb::open(&self.app_db)
      .expect("application database")
      .mcp_server_program(self.server, server)
      .expect("read")
      .map(|stored| stored.program)
  }

  /// A file a person could choose as a program.
  fn program_file(&self) -> PathBuf {
    let path = self.root.join("mcp-server");
    fs::write(&path, "#!/bin/sh\n").expect("a program");
    path
  }
}

fn pick(connection: ConnectionId) -> ConnectionPick {
  ConnectionPick {
    connection,
    account: None,
  }
}

#[test]
fn team_fill_ins_pick_a_connection_of_the_profiles_cli_kind() {
  let team = Team::new();
  let before = team.connection_changes();
  team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, pick(team.claude))
    .expect("a Claude Code connection for a Claude Code profile");
  assert_eq!(
    team.db().connection_pick(team.claude_profile).expect("read"),
    Some(pick(team.claude))
  );
  assert_eq!(team.connection_changes(), before + 1, "screens read again");

  let refused = team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, pick(team.codex));
  assert!(
    matches!(refused, Err(FillInError::ConnectionDoesNotFit { .. })),
    "{refused:?}"
  );
  assert_eq!(
    team.db().connection_pick(team.claude_profile).expect("read"),
    Some(pick(team.claude)),
    "the earlier pick stays"
  );
  assert_eq!(team.connection_changes(), before + 1, "a refusal changes nothing");
}

#[test]
fn team_fill_ins_a_profile_of_no_known_kind_takes_any_agent_connection() {
  let team = Team::new();
  for connection in [team.claude, team.codex] {
    team
      .workspace
      .pick_team_connection(&team.root, team.open_profile, pick(connection))
      .expect("an agent connection");
  }
  assert_eq!(
    team.db().connection_pick(team.open_profile).expect("read"),
    Some(pick(team.codex))
  );
}

#[test]
fn team_fill_ins_a_pick_names_a_team_profile_and_an_existing_connection() {
  let team = Team::new();
  let local = team
    .workspace
    .pick_team_connection(&team.root, team.local_profile, pick(team.claude));
  assert!(matches!(local, Err(FillInError::NotTeamResource { .. })), "{local:?}");
  let wrong_kind = team
    .workspace
    .pick_team_connection(&team.root, team.program_server, pick(team.claude));
  assert!(
    matches!(wrong_kind, Err(FillInError::NotTeamResource { .. })),
    "{wrong_kind:?}"
  );
  let missing = ConnectionId::generate();
  let gone = team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, pick(missing));
  assert!(
    matches!(gone, Err(FillInError::ConnectionNotFound(id)) if id == missing),
    "{gone:?}"
  );
  assert_eq!(team.db().connection_pick(team.local_profile).expect("read"), None);
}

#[test]
fn team_fill_ins_an_account_is_a_listed_account_of_the_connections_cli() {
  let team = Team::new();
  let with = |account| ConnectionPick {
    connection: team.claude,
    account: Some(account),
  };
  let default = AccountCli::ClaudeCode.default_account();
  team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, with(default))
    .expect("the Default account of Claude Code");
  assert_eq!(
    team.db().connection_pick(team.claude_profile).expect("read"),
    Some(with(default))
  );

  let other = AccountCli::Codex.default_account();
  let refused = team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, with(other));
  assert!(
    matches!(refused, Err(FillInError::AccountOfOtherCli(id)) if id == other),
    "{refused:?}"
  );
  let unknown = orchester_core::id::AgentAccountId::generate();
  let gone = team
    .workspace
    .pick_team_connection(&team.root, team.claude_profile, with(unknown));
  assert!(
    matches!(gone, Err(FillInError::AccountNotFound(id)) if id == unknown),
    "{gone:?}"
  );
}

#[test]
fn team_fill_ins_a_person_sets_the_program_of_a_team_mcp_server_and_clears_it() {
  let team = Team::new();
  let program = team.program_file();
  let before = team.connection_changes();
  team
    .workspace
    .set_team_mcp_program(&team.root, team.program_server, &program, Answerer::Person)
    .expect("a person chose the file");
  assert_eq!(team.program_set(team.program_server), Some(program));
  assert_eq!(team.connection_changes(), before + 1);

  team
    .workspace
    .clear_team_mcp_program(&team.root, team.program_server)
    .expect("cleared");
  assert_eq!(team.program_set(team.program_server), None);
  team
    .workspace
    .clear_team_mcp_program(&team.root, team.program_server)
    .expect("clearing again is not an error");
  assert_eq!(team.connection_changes(), before + 3);
}

#[test]
fn team_fill_ins_automation_cannot_set_a_program_but_can_clear_it() {
  let team = Team::new();
  let program = team.program_file();
  let refused = team
    .workspace
    .set_team_mcp_program(&team.root, team.program_server, &program, Answerer::Automation);
  assert!(matches!(refused, Err(FillInError::PersonOnly)), "{refused:?}");
  assert_eq!(team.program_set(team.program_server), None, "nothing changed");

  team
    .workspace
    .set_team_mcp_program(&team.root, team.program_server, &program, Answerer::Person)
    .expect("a person chose the file");
  team
    .workspace
    .clear_team_mcp_program(&team.root, team.program_server)
    .expect("clearing narrows, so anyone may");
  assert_eq!(team.program_set(team.program_server), None);
}

#[test]
fn team_fill_ins_a_program_is_an_absolute_path_to_a_file_of_a_local_team_server() {
  let team = Team::new();
  let set = |server, path: &std::path::Path| {
    team
      .workspace
      .set_team_mcp_program(&team.root, server, path, Answerer::Person)
  };
  let relative = set(team.program_server, std::path::Path::new("bin/mcp-server"));
  assert!(matches!(relative, Err(FillInError::ProgramNotAbsolute)), "{relative:?}");
  let missing = set(team.program_server, &team.root.join("nowhere"));
  assert!(matches!(missing, Err(FillInError::ProgramNotAFile)), "{missing:?}");
  let folder = set(team.program_server, &team.root);
  assert!(matches!(folder, Err(FillInError::ProgramNotAFile)), "{folder:?}");

  let program = team.program_file();
  let remote = set(team.remote_server, &program);
  assert!(
    matches!(remote, Err(FillInError::RemoteServer(id)) if id == team.remote_server),
    "{remote:?}"
  );
  let profile = set(team.claude_profile, &program);
  assert!(
    matches!(profile, Err(FillInError::NotTeamResource { .. })),
    "{profile:?}"
  );
  assert_eq!(team.program_set(team.program_server), None);
}

#[test]
fn team_fill_ins_a_person_turns_a_team_mcp_server_on_and_anyone_turns_it_off() {
  let team = Team::new();
  let server = team.program_server;
  let on = || team.db().team_mcp_on_here(server).expect("read");
  assert!(!on(), "off until a person turns it on");

  let refused = team
    .workspace
    .switch_team_mcp_on_here(&team.root, server, true, Answerer::Automation);
  assert!(matches!(refused, Err(FillInError::PersonOnly)), "{refused:?}");
  assert!(!on());

  let before = team.connection_changes();
  team
    .workspace
    .switch_team_mcp_on_here(&team.root, server, true, Answerer::Person)
    .expect("on");
  assert!(on());
  assert_eq!(team.connection_changes(), before + 1);

  team
    .workspace
    .switch_team_mcp_on_here(&team.root, server, false, Answerer::Automation)
    .expect("off narrows, so anyone may");
  assert!(!on());
}

#[test]
fn team_fill_ins_only_a_team_mcp_server_is_turned_on() {
  let team = Team::new();
  for not_a_server in [team.claude_profile, team.local_profile, ResourceId::generate()] {
    let refused = team
      .workspace
      .switch_team_mcp_on_here(&team.root, not_a_server, true, Answerer::Person);
    assert!(
      matches!(refused, Err(FillInError::NotTeamResource { .. })),
      "{refused:?}"
    );
    assert!(!team.db().team_mcp_on_here(not_a_server).expect("read"));
  }
}

#[test]
fn team_fill_ins_a_secret_value_is_set_by_name_in_the_keychain() {
  let team = Team::new();
  let vault = MemoryVault::default();
  let name = SecretName::new("STAGING_TOKEN").expect("name");
  let before = team.connection_changes();
  team
    .workspace
    .set_team_secret_value(
      &team.root,
      &vault,
      &name,
      &SecretValue::new(FAKE.to_owned()).expect("value"),
    )
    .expect("set");
  let db = team.db();
  assert!(db.has_team_secret_value(&name).expect("read"));
  let stored = db.team_secret_value(&vault, &name).expect("read").expect("a value");
  assert_eq!(stored.expose(), FAKE);
  assert_eq!(team.connection_changes(), before + 1);

  let database = fs::read(team.root.join(".orchester/orchester.db")).expect("database file");
  assert!(
    !database.windows(FAKE.len()).any(|window| window == FAKE.as_bytes()),
    "the project database keeps the name and a handle, never the value"
  );
}

#[test]
fn team_fill_ins_a_secret_value_never_reaches_debug_or_an_error() {
  let team = Team::new();
  let name = SecretName::new("STAGING_TOKEN").expect("name");
  let value = SecretValue::new(FAKE.to_owned()).expect("value");
  assert!(!format!("{value:?}").contains(FAKE));

  let locked = team
    .workspace
    .set_team_secret_value(&team.root, &LockedVault, &name, &value)
    .expect_err("the keychain refuses");
  let local = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(local.path().join(".git")).expect("repository");
  ProjectDb::create(
    local.path(),
    Project::new(
      ProjectId::generate(),
      ProjectKey::new("LOC").expect("key"),
      Name::new("Local").expect("name"),
    ),
  )
  .expect("a local project");
  let not_team = team
    .workspace
    .set_team_secret_value(local.path(), &MemoryVault::default(), &name, &value)
    .expect_err("a local project has no team secrets");
  assert!(matches!(not_team, FillInError::NotTeamProject), "{not_team:?}");

  for error in [locked, not_team] {
    let message = error.message();
    let shown = format!("{error} {error:?} {message:?}");
    assert!(!shown.contains(FAKE), "{shown}");
  }
  assert!(
    !team.db().has_team_secret_value(&name).expect("read"),
    "a refused value leaves nothing"
  );
}
