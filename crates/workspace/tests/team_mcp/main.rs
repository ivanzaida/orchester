//! The domain MCP tools on a team project (team authority P7, `orchester_workspace::app_mcp`) against a fake team
//! server on loopback with the server's routes and shapes (the work tests' [`team_server`], [`team_commands`] and
//! [`team_deliveries`], with the reads and definition routes of [`reads`]): what the tools read comes from the server,
//! a member's edit is refused `not_admin` and changes nothing here, `run_task` starts the task's work through
//! `work.start`, nothing falls back to the project database's tasks while the server is down, and a local project's
//! tools work as before ([`tools`]); `merge_run` of a team task's run requests its module's delivery and answers each
//! state it reaches, while a local project's `merge_run` keeps its own path ([`delivery`], P7b); a team task names
//! only a team pipeline, by its id, and a team project offers no pipeline of this computer's own ([`pipelines`]).
//!
//! `cargo test -p orchester-workspace --test team_mcp team_mcp_`

#[path = "../team_work/fixture.rs"]
mod fixture;
#[path = "../actions/http.rs"]
mod http;
#[path = "../team_work/team_commands.rs"]
mod team_commands;
#[path = "../team_work/team_deliveries.rs"]
mod team_deliveries;
#[path = "../team_work/team_server.rs"]
mod team_server;

mod bundle_missing;
mod bundle_schema;
mod choices;
mod created;
mod delivery;
mod mcp_server_remove;
mod mcp_servers;
mod pipelines;
mod readiness;
mod reads;
mod share;
mod tools;

use std::{
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
  process::Command,
  sync::Arc,
};

use orchester_api::client::{PlainHttp, ServerAddress};
use orchester_core::{
  connection::{AgentConnection, AgentProvider},
  id::{ConnectionId, IdempotencyKey, ProjectId, ServerId, UserId},
  name::Name,
  project::{Project, ProjectKey},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::{ConnectionStore, TaskStore},
  task::{TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_store::{AppDb, ProjectDb, ServerAccount};
use orchester_workspace::{
  activity::Activity,
  app_mcp::{self, Translate},
  runner::Runners,
  secrets::Vault,
  sharing::Servers,
  tasks::{integration_jobs::ProjectJobs, outside::OutsideChanges},
  workspace::Workspace,
};
use parking_lot::Mutex;
use serde_json::Value;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// The keychain of these tests: the server tokens stored in it, in memory.
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

/// A project named Kontur in a git repository on `main` with a `module/webhooks` branch, a Codex connection and local
/// drafts titled `drafts` (`KON-1` on), registered in a workspace whose app MCP tools the tests call; linked to
/// `server`'s project when one is given, after the drafts were saved.
struct Checkout {
  dir: TempDir,
  services: app_mcp::Services,
  workspace: Workspace,
  runtime: Runtime,
  _data: TempDir,
}

impl Checkout {
  fn new(server: Option<&team_server::TeamServer>, drafts: &[&str]) -> Self {
    Self::prepared(server, drafts, |_| {})
  }

  /// [`Self::new`], with what `prepare` keeps in the project's database before it is linked.
  fn prepared(server: Option<&team_server::TeamServer>, drafts: &[&str], prepare: impl FnOnce(&mut ProjectDb)) -> Self {
    Self::standing(server, (drafts, Standing::default()), prepare)
  }

  /// [`Self::prepared`], linked as `standing` says. The computer is not signed in to the server: what it publishes
  /// waits to be sent.
  fn standing(
    server: Option<&team_server::TeamServer>,
    (drafts, standing): (&[&str], Standing),
    prepare: impl FnOnce(&mut ProjectDb),
  ) -> Self {
    Self::build(server, (drafts, standing), prepare, false)
  }

  /// A checkout linked as `standing` says, with an account signed in to `server`: what it publishes reaches the server.
  fn signed_in(server: &team_server::TeamServer, standing: Standing) -> Self {
    Self::build(Some(server), (&[], standing), |_| {}, true)
  }

  fn build(
    server: Option<&team_server::TeamServer>,
    (drafts, standing): (&[&str], Standing),
    prepare: impl FnOnce(&mut ProjectDb),
    signed_in: bool,
  ) -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/team-mcp-tests");
    fs::create_dir_all(&scratch).expect("scratch");
    let dir = tempfile::Builder::new()
      .prefix("project-")
      .tempdir_in(&scratch)
      .expect("project");
    let root = dir.path();
    repository(root);
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("KON").expect("key"),
      Name::new("Kontur").expect("name"),
    );
    let mut db = ProjectDb::create(root, project).expect("create project");
    let codex = AgentConnection::new(
      ConnectionId::generate(),
      Name::new("Codex").expect("name"),
      AgentProvider::CodexCli,
      root.join("codex.exe"),
      Timestamp::now(),
    )
    .expect("connection");
    db.create_connection(&codex).expect("a connection");
    if !drafts.is_empty() {
      let details = drafts.iter().map(|title| draft(title)).collect();
      db.create_tasks(IdempotencyKey::generate(), details, Timestamp::now())
        .expect("the drafts");
    }
    prepare(&mut db);
    drop(db);
    let data = tempfile::Builder::new()
      .prefix("data-")
      .tempdir_in(&scratch)
      .expect("data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    workspace.open(root).expect("register the project");
    workspace
      .set_automation_changes_permissions(standing.widens)
      .expect("the switch");
    let runtime = Runtime::new().expect("runtime");
    let vault = Arc::new(MemoryVault::default());
    let version = orchester_api::releases::ReleaseVersion::parse("0.5.0").expect("a release version");
    let servers = Servers::new(workspace.clone(), Vault::with(vault.clone()), version);
    if let Some(server) = server {
      let id = runtime.block_on(add_server(&servers, server));
      if signed_in {
        sign_in(&data.path().join("orchester.db"), id);
      }
      let token = SecretValue::new(team_server::MEMBER.to_owned()).expect("token");
      vault.store(&VaultKey::server_token(id), &token).expect("the token");
      link(root, id, server.project, standing.role);
    }
    let runners = Runners::new(
      root.join("no-runner"),
      runtime.handle().clone(),
      Activity::new(),
      orchester_ipc::SessionEnd::Clean,
    );
    let services = app_mcp::Services::new(
      workspace.clone(),
      runners,
      servers,
      Programs::processes(),
      (OutsideChanges::default(), Activity::new(), ProjectJobs::default()),
    );
    Self {
      dir,
      services,
      workspace,
      runtime,
      _data: data,
    }
  }

  fn root(&self) -> &Path {
    self.dir.path()
  }

  fn db(&self) -> ProjectDb {
    ProjectDb::open(self.root()).expect("open")
  }

  /// Tool `name`, a domain or a setup tool, called with `args`: its result, or its error as the client reads it.
  fn call(&self, name: &str, args: &Value) -> Result<Value, Value> {
    self.call_rendering(name, args, |message| message.key.to_owned())
  }

  /// [`Self::call`], with each message rendered by `render` (to read its placeholders' values).
  fn call_rendering(
    &self,
    name: &str,
    args: &Value,
    render: impl Fn(&orchester_workspace::messages::Message) -> String + Copy + Send + Sync + 'static,
  ) -> Result<Value, Value> {
    let translate = Translate::new(render);
    let Value::Object(args) = args.clone() else {
      panic!("arguments are an object");
    };
    let services = &self.services;
    self
      .runtime
      .block_on(async {
        match app_mcp::call(services, &translate, name, args.clone()).await {
          Some(result) => Some(result),
          None => app_mcp::setup::call(services, &translate, name, args).await,
        }
      })
      .expect("a tool")
      .map_err(|error| error.to_json(render))
  }

  /// The error code of tool `name`'s refusal of `args`.
  fn refused(&self, name: &str, args: &Value) -> String {
    let error = self.call(name, args).expect_err("refused");
    error["error"]["code"].as_str().expect("a code").to_owned()
  }
}

/// `server` added to the workspace's servers, plain HTTP on loopback allowed.
async fn add_server(servers: &Servers, server: &team_server::TeamServer) -> ServerId {
  let address = ServerAddress::parse(&server.url).expect("address");
  servers
    .add(&address, "acme".to_owned(), PlainHttp::Allowed)
    .await
    .expect("the server added")
}

/// Records an account as signed in to `server` in the application database at `path`.
fn sign_in(path: &Path, server: ServerId) {
  let account = ServerAccount {
    user: UserId::generate(),
    username: "dana".to_owned(),
    display_name: "Dana".to_owned(),
    admin: true,
    signed_in_at: Timestamp::now(),
  };
  AppDb::open(path)
    .expect("the application database")
    .set_server_account(server, Some(&account))
    .expect("the account");
}

/// How a checkout stands with its team: the person's role in the link, and whether a person lets automation change
/// agent permissions.
pub(crate) struct Standing {
  pub(crate) role: &'static str,
  pub(crate) widens: bool,
}

impl Default for Standing {
  fn default() -> Self {
    Self {
      role: "member",
      widens: false,
    }
  }
}

/// Links the project at `root` to `project` on `server`, with the person's `role` there.
fn link(root: &Path, server: ServerId, project: ProjectId, role: &str) {
  rusqlite::Connection::open(root.join(".orchester/orchester.db"))
    .expect("open")
    .execute(
      "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, \
       last_synced_at) VALUES (1, ?1, ?2, 'KON', ?3, 1, 0, 1)",
      [server.to_string(), project.to_string(), role.to_owned()],
    )
    .expect("link the project");
}

/// A draft titled `title`, without pipeline or limits.
fn draft(title: &str) -> TaskDetails {
  TaskDetails {
    title: Name::new(title).expect("name"),
    brief: "Sign in with a password.".to_owned(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  }
}

/// A repository at `root` on `main` with one commit, and the module's branch one commit ahead of it, as a task run
/// merged into it left it (D3).
fn repository(root: &Path) {
  git(root, &["init", "-b", "main"]);
  git(root, &["config", "user.name", "Team MCP test"]);
  git(root, &["config", "user.email", "test@example.invalid"]);
  git(root, &["config", "core.autocrlf", "false"]);
  fs::write(root.join(".gitignore"), ".orchester/\n").expect("ignore");
  fs::write(root.join("README.md"), "Kontur\n").expect("readme");
  git(root, &["add", "."]);
  git(root, &["commit", "-m", "base"]);
  git(root, &["switch", "-c", team_server::MODULE_BRANCH]);
  fs::write(root.join("webhooks.txt"), "webhooks\n").expect("webhooks");
  git(root, &["add", "webhooks.txt"]);
  git(root, &["commit", "-m", "Merge KON-7 run #1"]);
  git(root, &["switch", "main"]);
}

/// Git's trimmed output of `args` at `root`.
fn git(root: &Path, args: &[&str]) -> String {
  let output = Command::new("git").args(args).current_dir(root).output().expect("git");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  String::from_utf8(output.stdout).expect("text").trim().to_owned()
}
