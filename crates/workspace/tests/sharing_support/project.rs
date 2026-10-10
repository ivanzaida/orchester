//! A person's desktop with a local project, signed in to a real `orchester-server` on loopback, for
//! the shared-project tests: its servers on a temporary application database, a keychain in memory
//! and a runtime of its own on which the service's blocking work waits.

#![allow(dead_code, reason = "shared by the shared-project tests, each of which uses a part")]

use std::{collections::HashMap, path::Path, sync::Arc};

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
    ServerEntry, Servers, SignedIn,
    project::{ProjectServer, store::RemoteTaskStore},
  },
  workspace::Workspace,
};
use parking_lot::Mutex;
use tempfile::TempDir;
use tokio::runtime::Runtime;

use crate::server_support::{SERVER_NAME, TestServer};

/// The repository every test shares.
pub const REMOTE: &str = "https://github.com/ivanzaida/kontur.git";

/// A keychain in memory: tests never touch the keychain of the user running them.
#[derive(Default)]
pub struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::Failed("blank".to_owned()))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

/// One person's computer: the desktop's servers and a local project Kontur (key ORC).
pub struct Person {
  pub servers: Servers,
  pub runtime: Runtime,
  pub repo: TempDir,
  _data: TempDir,
}

impl Person {
  /// `username`, signed in to `server` with their own password, with a local project of `titles`.
  pub fn signed_in(server: &TestServer, (username, first): (&str, &Credential), titles: &[&str]) -> Self {
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
    Self::with_repo(server, (username, first), repo)
  }

  /// `username`, signed in to `server` with their own password, with the project in `repo`.
  pub fn with_repo(server: &TestServer, (username, first): (&str, &Credential), repo: TempDir) -> Self {
    let data = TempDir::new().expect("application data");
    let workspace = Workspace::new(AppDb::open(&data.path().join("orchester.db")).expect("database"));
    let servers = Servers::new(workspace.clone(), Vault::with(Arc::new(MemoryVault::default())));
    workspace.open(repo.path()).expect("register the project");
    let person = Self {
      servers,
      runtime: Runtime::new().expect("desktop runtime"),
      repo,
      _data: data,
    };
    person.sign_in(server, username, first);
    person
  }

  fn sign_in(&self, server: &TestServer, username: &str, first: &Credential) {
    let address = ServerAddress::parse(&server.url()).expect("address");
    self
      .block_on(self.servers.add(&address, SERVER_NAME.into(), PlainHttp::NotAllowed))
      .expect("add the server");
    let entry = self.entry();
    let signed = self
      .block_on(
        self
          .servers
          .sign_in(&entry, username.into(), Credential::new(first.expose().into())),
      )
      .expect("sign in");
    let SignedIn::PasswordChangeRequired(pending) = signed else {
      panic!("a first password asks for the person's own");
    };
    self
      .block_on(self.servers.choose_password(
        &entry,
        &pending,
        Credential::new(first.expose().into()),
        Credential::new(format!("{username}'s own long password")),
      ))
      .expect("choose a password");
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

  /// Shares the project on the server.
  pub fn share(&self) {
    let identity = RepositoryIdentity::from_remote_url(REMOTE).expect("identity");
    self
      .block_on(
        self
          .servers
          .share(&self.entry(), self.root(), identity, IdempotencyKey::generate(), true),
      )
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
    let token = server.sign_in_elsewhere(username, first.expose(), &format!("{username}'s own long password"));
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
  let first = server.create_user("alice", "Alice Moreau", true);
  let alice = Person::signed_in(
    server,
    ("alice", &first),
    &["Repository switcher", "Fix reconnect", "Merge queue"],
  );
  alice.share();
  alice
}

/// Adds `other` to Alice's project as a member.
pub fn member(alice: &Person, username: &str) {
  alice
    .block_on(alice.server().add_member(username.into(), MemberRole::Member))
    .expect("add a member");
}
