//! A person's desktop with a local project, joined to a real `orchester-server` on loopback, for
//! the shared-project tests: its servers on a temporary application database, a keychain in memory
//! and a runtime of its own on which the service's blocking work waits.

#![allow(dead_code, reason = "shared by the shared-project tests, each of which uses a part")]

use std::{
  collections::HashMap,
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use orchester_api::{
  Credential, MemberRole,
  client::{Client, PlainHttp, ServerAddress},
};
use orchester_core::{
  id::{IdempotencyKey, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  repository::RepositoryIdentity,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::TaskStore,
  task::{Task, TaskDetails, TaskLimits},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  secrets::Vault,
  sharing::{
    CodeRequest, ServerEntry, Servers, Trust,
    project::{LinkFirst, ProjectServer, store::RemoteTaskStore},
  },
  workspace::Workspace,
};
use parking_lot::Mutex;
use tempfile::TempDir;
use tokio::runtime::Runtime;

use crate::server_support::TestServer;

/// The repository every test shares.
pub const REMOTE: &str = "https://github.com/ivanzaida/kontur.git";

/// A keychain in memory: tests never touch the keychain of the user running them. It can refuse reads and removals,
/// as a keychain whose access the person denied does.
#[derive(Default)]
pub struct MemoryVault {
  values: Mutex<HashMap<VaultKey, String>>,
  refuses_reads: AtomicBool,
  refuses_deletes: AtomicBool,
}

impl MemoryVault {
  /// From now on, reads and removals answer [`VaultError::AccessDenied`] as `reads` and `deletes` say.
  pub fn refuse(&self, reads: bool, deletes: bool) {
    self.refuses_reads.store(reads, Ordering::SeqCst);
    self.refuses_deletes.store(deletes, Ordering::SeqCst);
  }

  /// Whether a value is stored under `key`, whatever reads answer.
  pub fn holds(&self, key: &VaultKey) -> bool {
    self.values.lock().contains_key(key)
  }
}

fn denied() -> VaultError {
  VaultError::AccessDenied("the person denied access".to_owned())
}

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.values.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    if self.refuses_reads.load(Ordering::SeqCst) {
      return Err(denied());
    }
    let value = self.values.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    if self.refuses_deletes.load(Ordering::SeqCst) {
      return Err(denied());
    }
    self.values.lock().remove(key);
    Ok(())
  }
}

/// One person's computer: the desktop's servers and a local project Kontur (key ORC).
pub struct Person {
  pub servers: Servers,
  /// The application's projects, as the domain tools find them.
  pub workspace: Workspace,
  pub runtime: Runtime,
  pub repo: TempDir,
  _data: TempDir,
}

impl Person {
  /// `(username, display name, code)`, joined to `server` from this computer with the code (a setup or invite code),
  /// with a local project of `titles`.
  pub fn signed_in(server: &TestServer, person: (&str, &str, &Credential), titles: &[&str]) -> Self {
    let repo = TempDir::new().expect("repository");
    std::fs::create_dir(repo.path().join(".git")).expect("create .git");
    let project = Project::new(
      ProjectId::generate(),
      ProjectKey::new("ORC").expect("key"),
      Name::new("Kontur").expect("name"),
    );
    let mut db = ProjectDb::create(repo.path(), project).expect("project");
    if !titles.is_empty() {
      db.create_tasks(
        IdempotencyKey::generate(),
        titles.iter().map(|title| details(title)).collect(),
        Timestamp::now(),
      )
      .expect("tasks");
    }
    drop(db);
    Self::with_repo(server, person, repo)
  }

  /// `username`, joined to `server` from this computer with `code`, with the project in `repo`.
  pub fn with_repo(server: &TestServer, person: (&str, &str, &Credential), repo: TempDir) -> Self {
    let data = TempDir::new().expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    let servers = Servers::new(workspace.clone(), Vault::with(Arc::new(MemoryVault::default())));
    workspace.open(repo.path()).expect("register the project");
    let desktop = Self {
      servers,
      workspace,
      runtime: Runtime::new().expect("desktop runtime"),
      repo,
      _data: data,
    };
    desktop.join(server, person);
    desktop
  }

  /// Joins as `username`, called `display_name`, with `code`.
  fn join(&self, server: &TestServer, (username, display_name, code): (&str, &str, &Credential)) {
    let address = ServerAddress::parse(&server.url()).expect("address");
    let request = CodeRequest {
      code: Credential::new(code.expose().into()),
      username: Some(username.into()),
      display_name: Some(display_name.into()),
      proof: Some(server.code_proof(username)),
    };
    self
      .block_on(
        self
          .servers
          .join_with_code(&address, Trust::System(PlainHttp::NotAllowed), request),
      )
      .expect("join with a code");
  }

  pub fn block_on<T>(&self, work: impl Future<Output = T>) -> T {
    self.runtime.block_on(work)
  }

  pub fn root(&self) -> &Path {
    self.repo.path()
  }

  pub fn entry(&self) -> ServerEntry {
    self.servers.entries().expect("entries").remove(0)
  }

  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(self.root()).expect("project database")
  }

  pub fn tasks(&self) -> Vec<Task> {
    self.db().tasks().expect("tasks")
  }

  /// Links the project, as its first checkout, to the empty project `kontur` the server's admin made for the
  /// repository: every task keeps its number.
  pub fn share(&self, server: &TestServer) {
    server.create_project("kontur", Some(REMOTE));
    let request = LinkFirst {
      slug: "kontur".into(),
      remote: Some(RepositoryIdentity::from_remote_url(REMOTE).expect("identity")),
      key: IdempotencyKey::generate(),
      write_discovery: true,
    };
    self
      .block_on(self.servers.link_first(&self.entry(), self.root(), request))
      .expect("share");
  }

  pub fn server(&self) -> ProjectServer {
    ProjectServer::open(&self.servers, self.root())
      .expect("sharing")
      .expect("shared")
  }

  /// Runs `work` with the shared project's store, on a thread that waits for its requests.
  pub fn with_store<T>(&self, work: impl FnOnce(&mut RemoteTaskStore) -> T) -> T {
    let _context = self.runtime.enter();
    let mut store = RemoteTaskStore::new(self.db(), self.server()).expect("store");
    work(&mut store)
  }
}

pub fn details(title: &str) -> TaskDetails {
  TaskDetails {
    title: Name::new(title).expect("title"),
    brief: String::new(),
    acceptance_criteria: Vec::new(),
    pipeline: None,
    limits: TaskLimits::NONE,
  }
}

/// Another member's computer, which talks to the server's API directly.
pub struct Other {
  pub client: Client,
  pub token: SecretValue,
}

impl Other {
  pub fn new(server: &TestServer, username: &str, name: &str) -> Self {
    let first = server.create_user(username, name, false);
    let token = SecretValue::new(first.expose().to_owned()).expect("a token");
    let client = Client::new(
      ServerAddress::parse(&server.url()).expect("address"),
      PlainHttp::NotAllowed,
    )
    .expect("client");
    Self { client, token }
  }
}

/// Alice, the server's first admin, with a local project of three tasks, shared.
pub fn shared(server: &TestServer) -> Person {
  let code = server.code_for("alice", true);
  let alice = Person::signed_in(
    server,
    ("alice", "Alice Moreau", &code),
    &["Repository switcher", "Fix reconnect", "Merge queue"],
  );
  alice.share(server);
  alice
}

/// Adds `other` to Alice's project as a member.
pub fn member(alice: &Person, username: &str) {
  alice
    .block_on(alice.server().add_member(username.into(), MemberRole::Member))
    .expect("add a member");
}
