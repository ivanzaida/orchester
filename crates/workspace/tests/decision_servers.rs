//! The person's decision model servers (`docs/design/agent-accounts.md`, "Agents run on accounts", decision 3): a
//! server is added once for every project, with its key in the app's keychain entry; a project has a copy of it among
//! its decision connections, by the same id, which holds no secret and follows the server.
//! `cargo test -p orchester-workspace --test decision_servers decision_servers`.

use std::{collections::HashMap, fs};

use orchester_core::{
  connection::{DecisionCheckResult, DecisionConnection, DecisionServerUrl},
  id::{ApiAccountId, DecisionConnectionId, ProjectId, SecretId},
  name::Name,
  project::{Project, ProjectKey},
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::SecretStore,
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{accounts::ServerKey, project_setup::NewProject, workspace::Workspace};
use parking_lot::Mutex;
use tempfile::TempDir;

const KEY: &str = "sk-decision-typed-by-the-test";

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

struct Fixture {
  _data: TempDir,
  repo: TempDir,
  workspace: Workspace,
  vault: MemoryVault,
}

fn name(text: &str) -> Name {
  Name::new(text).expect("a name")
}

fn url(text: &str) -> DecisionServerUrl {
  DecisionServerUrl::new(text).expect("a URL")
}

/// A local project with no decision connection.
fn fixture() -> Fixture {
  let data = tempfile::tempdir().expect("temporary directory");
  let db = AppDb::open(&data.path().join("orchester.db")).expect("the app database");
  let workspace = Workspace::new(db).with_account_root(&data.path().join("accounts"));
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  workspace
    .create(NewProject {
      project: Project::new(
        ProjectId::generate(),
        ProjectKey::new("KON").expect("valid key"),
        name("Kontur"),
      ),
      repo_root: repo.path().to_path_buf(),
    })
    .expect("created");
  Fixture {
    _data: data,
    repo,
    workspace,
    vault: MemoryVault::default(),
  }
}

/// *Laya*, a hosted decision model server that takes a key.
fn laya(fixture: &Fixture) -> DecisionConnection {
  fixture
    .workspace
    .add_decision_server(
      &fixture.vault,
      name("Laya"),
      url("https://api.laya.studio/v1"),
      Some(&SecretValue::new(KEY.to_owned()).expect("a key")),
    )
    .expect("added")
}

fn copies(fixture: &Fixture) -> Vec<DecisionConnection> {
  ProjectDb::open(fixture.repo.path())
    .expect("database")
    .decision_connections()
    .expect("read")
}

fn entry(id: DecisionConnectionId) -> VaultKey {
  VaultKey::api_account(ApiAccountId::from_uuid(id.as_uuid()))
}

/// Adding a server keeps its key in the app's entry under the server's id and gives the project a copy of it by the
/// same id, which names that id as its key's and holds no secret; nothing of the key is in either database.
#[test]
fn decision_servers_a_server_is_added_once_and_every_project_gets_its_copy() {
  let fixture = fixture();

  let server = laya(&fixture);

  assert_eq!(
    fixture.workspace.decision_servers().expect("read"),
    std::slice::from_ref(&server)
  );
  assert_eq!(server.key(), Some(SecretId::from_uuid(server.id().as_uuid())));
  assert_eq!(
    fixture.vault.read(&entry(server.id())).expect("kept").expose(),
    KEY,
    "the key is the app's"
  );
  let listed = copies(&fixture);
  assert_eq!(
    listed,
    std::slice::from_ref(&server),
    "the project's copy is the server"
  );
  let db = ProjectDb::open(fixture.repo.path()).expect("database");
  assert!(
    db.secrets().expect("read").is_empty(),
    "the project holds no secret of it"
  );
  assert!(!format!("{server:?}{listed:?}").contains(KEY));
}

/// A name another server has is refused, and the key typed for the one refused does not stay in the keychain.
#[test]
fn decision_servers_a_taken_name_is_refused_and_its_key_is_not_kept() {
  let fixture = fixture();
  laya(&fixture);

  let refused = fixture.workspace.add_decision_server(
    &fixture.vault,
    name("laya"),
    url("http://127.0.0.1:9000/v1"),
    Some(&SecretValue::new("another".to_owned()).expect("a key")),
  );

  assert!(refused.is_err(), "{refused:?}");
  assert_eq!(fixture.workspace.decision_servers().expect("read").len(), 1);
  assert_eq!(fixture.vault.0.lock().len(), 1, "only the first server's key is kept");
}

/// A change in Settings reaches the project's copy: the name and the server; another base URL drops what the old one
/// listed. A key removed leaves the entry empty and the copy with no key.
#[test]
fn decision_servers_a_copy_follows_its_server() {
  let fixture = fixture();
  let server = laya(&fixture);

  let changed = fixture
    .workspace
    .change_decision_server(
      &fixture.vault,
      server.id(),
      (name("Laya EU"), url("https://eu.laya.studio/v1")),
      ServerKey::Keep,
    )
    .expect("changed");

  assert_eq!(copies(&fixture), std::slice::from_ref(&changed));
  assert_eq!(changed.name().as_str(), "Laya EU");
  assert_eq!(changed.key(), server.key(), "the key stays");

  let keyless = fixture
    .workspace
    .change_decision_server(
      &fixture.vault,
      server.id(),
      (name("Laya EU"), url("https://eu.laya.studio/v1")),
      ServerKey::Remove,
    )
    .expect("changed");

  assert_eq!(keyless.key(), None);
  assert_eq!(copies(&fixture), [keyless]);
  assert!(fixture.vault.read(&entry(server.id())).is_err(), "its key is gone");
}

/// A decision connection the project made itself keeps its name: the copy of a server of the same name is numbered.
#[test]
fn decision_servers_a_copy_is_numbered_beside_a_connection_of_the_projects_own_by_that_name() {
  let fixture = fixture();
  let own = DecisionConnection::new(
    DecisionConnectionId::generate(),
    name("Laya"),
    url("http://127.0.0.1:9000/v1"),
    None,
  );
  ProjectDb::open(fixture.repo.path())
    .expect("database")
    .create_decision_connection(&own)
    .expect("the project's own");

  let server = laya(&fixture);

  let listed = copies(&fixture);
  let copy = listed.iter().find(|found| found.id() == server.id()).expect("the copy");
  assert_eq!(copy.name().as_str(), "Laya (2)");
  assert!(listed.contains(&own), "the project's own stays as it is");
}

/// Removing a server takes its key and the copies no reviewer uses; a project that reviews with it keeps its copy.
#[test]
fn decision_servers_removing_one_takes_its_key_and_the_copies_no_reviewer_uses() {
  let fixture = fixture();
  let unused = laya(&fixture);
  let used = fixture
    .workspace
    .add_decision_server(&fixture.vault, name("Local"), url("http://127.0.0.1:9000/v1"), None)
    .expect("added");
  ProjectDb::open(fixture.repo.path())
    .expect("database")
    .save_decision_reviewer_connection(Some(used.id()))
    .expect("the reviewer's");

  fixture
    .workspace
    .remove_decision_server(&fixture.vault, unused.id())
    .expect("removed");
  fixture
    .workspace
    .remove_decision_server(&fixture.vault, used.id())
    .expect("removed");

  assert!(fixture.workspace.decision_servers().expect("read").is_empty());
  assert!(fixture.vault.read(&entry(unused.id())).is_err(), "its key is gone");
  assert_eq!(copies(&fixture), [used], "the reviewer's copy stays");
}

/// A check asks the server for its models and keeps the answer on the server and on every copy; where nothing
/// answers, that is what is kept, and no model.
#[tokio::test]
async fn decision_servers_a_check_is_kept_on_the_server_and_reaches_the_copy() {
  let fixture = fixture();
  let server = fixture
    .workspace
    // Port 1 of this computer: nothing answers there.
    .add_decision_server(&fixture.vault, name("Local"), url("http://127.0.0.1:1/v1"), None)
    .expect("added");

  let checked = fixture
    .workspace
    .check_decision_server(&fixture.vault, server.id())
    .await
    .expect("checked");

  let check = checked.last_check().expect("a check");
  assert_eq!(check.result, DecisionCheckResult::NothingAnswers);
  assert!(checked.models().is_empty());
  assert_eq!(copies(&fixture), [checked]);
}
