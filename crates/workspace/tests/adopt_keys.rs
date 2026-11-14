//! What a project held of model APIs before accounts were the app's becomes the app's when it opens
//! (`docs/design/agent-accounts.md`, "Agents run on accounts", decisions 3 and 6): a connection on a key of the
//! project's becomes the copy of an API account with the key secret's id, two projects on the same API with the same
//! key share one account, and a decision connection becomes a server of the app by its own id. Nothing of the
//! project's is removed. `cargo test -p orchester-workspace --test adopt_keys adopt_keys`.

use std::{collections::HashMap, fs, path::Path};

use orchester_core::{
  account::ApiSetup,
  connection::{
    AgentConnection, AgentProvider, BaseUrl, DecisionConnection, DecisionServerUrl, EndpointPreset, NativeModels,
  },
  id::{ApiAccountId, DecisionConnectionId, ProjectId, SecretId},
  name::Name,
  project::{Project, ProjectKey},
  secret::{self, Environment, NewSecret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  store::{ConnectionStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  connections::{
    data::agent_connections,
    native::{KeySecret, NativeDraft, save_native_in},
  },
  project_setup::NewProject,
  workspace::Workspace,
};
use parking_lot::Mutex;
use tempfile::TempDir;

const KEY: &str = "sk-ant-held-by-the-project";

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
  workspace: Workspace,
  vault: MemoryVault,
}

fn name(text: &str) -> Name {
  Name::new(text).expect("a name")
}

fn fixture() -> Fixture {
  let data = tempfile::tempdir().expect("temporary directory");
  let db = AppDb::open(&data.path().join("orchester.db")).expect("the app database");
  Fixture {
    workspace: Workspace::new(db).with_account_root(&data.path().join("accounts")),
    _data: data,
    vault: MemoryVault::default(),
  }
}

/// A local project named `title` with no connection.
fn project(fixture: &Fixture, (key, title): (&str, &str)) -> TempDir {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  fixture
    .workspace
    .create(NewProject {
      project: Project::new(
        ProjectId::generate(),
        ProjectKey::new(key).expect("valid key"),
        name(title),
      ),
      repo_root: repo.path().to_path_buf(),
    })
    .expect("created");
  repo
}

/// A native connection of the project at `root` on Anthropic's API, its key `value` in a new secret of the project.
fn native(fixture: &Fixture, root: &Path, value: &str) -> AgentConnection {
  let draft = NativeDraft {
    id: None,
    name: name("Anthropic API"),
    provider: AgentProvider::AnthropicApi,
    preset: EndpointPreset::Official,
    base_url: BaseUrl::new("https://api.anthropic.com").expect("a base URL"),
    key: KeySecret::New(NewSecret {
      name: SecretName::new("ANTHROPIC_API_KEY").expect("a valid name"),
      kind: SecretKind::ApiKey,
      environment: Environment::NEW_SECRET,
    }),
    models: NativeModels::default(),
    provider_fallback: false,
  };
  let id = save_native_in(
    root,
    &fixture.vault,
    &draft,
    Some(SecretValue::new(value.to_owned()).expect("a key")),
  )
  .expect("saved");
  connections(root)
    .into_iter()
    .find(|connection| connection.id() == id)
    .expect("stored")
}

fn connections(root: &Path) -> Vec<AgentConnection> {
  agent_connections(ProjectDb::open(root).expect("database").connections().expect("read"))
}

/// A project's native connection becomes the copy of an API account that has the id of its key secret and the key's
/// value in the app's entry; the connection still names that id, and the project's secret stays with its value.
#[test]
fn adopt_keys_a_connection_on_a_project_key_becomes_the_copy_of_an_api_account() {
  let fixture = fixture();
  let repo = project(&fixture, ("KON", "Kontur"));
  let before = native(&fixture, repo.path(), KEY);
  let key = before.api().expect("native").key();

  fixture
    .workspace
    .adopt_project_keys(repo.path(), &fixture.vault)
    .expect("adopted");
  fixture
    .workspace
    .adopt_project_keys(repo.path(), &fixture.vault)
    .expect("a second time changes nothing");

  let accounts = fixture.workspace.api_accounts().expect("read");
  let [account] = accounts.as_slice() else {
    panic!("one account: {accounts:?}");
  };
  assert_eq!(account.id(), ApiAccountId::from_uuid(key.as_uuid()), "the secret's id");
  assert_eq!(account.name().as_str(), "Anthropic API");
  assert!(
    matches!(account.setup(), ApiSetup::Native { .. }),
    "{:?}",
    account.setup()
  );
  assert_eq!(
    fixture
      .vault
      .read(&VaultKey::api_account(account.id()))
      .expect("the app's entry")
      .expose(),
    KEY
  );
  assert_eq!(connections(repo.path()), [before], "the connection is as it was");
  let db = ProjectDb::open(repo.path()).expect("database");
  let secret = db.secret(key).expect("read").expect("the project's secret stays");
  assert!(!secret.is_archived());
  assert!(!format!("{accounts:?}").contains(KEY));
}

/// Two projects on the same API with the same key are one account: the second project's connection goes onto the
/// account the first made. A project on another key gets an account of its own, named after the project.
#[test]
fn adopt_keys_the_same_api_and_key_in_two_projects_is_one_account() {
  let fixture = fixture();
  let (first, second, third) = (
    project(&fixture, ("KON", "Kontur")),
    project(&fixture, ("BIL", "Billing")),
    project(&fixture, ("OPS", "Ops")),
  );
  let one = native(&fixture, first.path(), KEY);
  native(&fixture, second.path(), KEY);
  native(&fixture, third.path(), "sk-ant-another-key");
  for repo in [&first, &second, &third] {
    fixture
      .workspace
      .adopt_project_keys(repo.path(), &fixture.vault)
      .expect("adopted");
  }

  let accounts = fixture.workspace.api_accounts().expect("read");
  let names: Vec<&str> = accounts.iter().map(|account| account.name().as_str()).collect();
  assert_eq!(names, ["Anthropic API", "Anthropic API (Ops)"]);
  let shared = one.api().expect("native").key();
  let moved = connections(second.path());
  assert_eq!(
    moved[0].api().expect("native").key(),
    shared,
    "the second project's connection is on the first's account"
  );
}

/// A connection whose key this computer's keychain does not hold is left as it was, and no account is made.
#[test]
fn adopt_keys_a_key_that_is_not_on_this_computer_stays_the_projects() {
  let fixture = fixture();
  let repo = project(&fixture, ("KON", "Kontur"));
  let before = native(&fixture, repo.path(), KEY);
  fixture.vault.0.lock().clear();

  fixture
    .workspace
    .adopt_project_keys(repo.path(), &fixture.vault)
    .expect("nothing to adopt is no failure");

  assert!(fixture.workspace.api_accounts().expect("read").is_empty());
  assert_eq!(connections(repo.path()), [before]);
}

/// A project's decision connection becomes a decision model server of the app by its own id, so the project's choice
/// of reviewer stays; its key is copied into the server's entry, and the connection names the server's id as its
/// key's.
#[test]
fn adopt_keys_a_decision_connection_becomes_a_server_of_the_app_by_its_own_id() {
  let fixture = fixture();
  let repo = project(&fixture, ("KON", "Kontur"));
  let id = DecisionConnectionId::generate();
  {
    let mut db = ProjectDb::open(repo.path()).expect("database");
    let project = db.project().id();
    let held = secret::add(
      &mut db,
      &fixture.vault,
      project,
      NewSecret {
        name: SecretName::new("DECISION_KEY").expect("a valid name"),
        kind: SecretKind::ApiKey,
        environment: Environment::NEW_SECRET,
      },
      &SecretValue::new(KEY.to_owned()).expect("a key"),
      Timestamp::now(),
    )
    .expect("the project's secret");
    let connection = DecisionConnection::new(
      id,
      name("Laya"),
      DecisionServerUrl::new("https://api.laya.studio/v1").expect("a URL"),
      Some(held.id()),
    );
    db.create_decision_connection(&connection).expect("created");
    db.save_decision_reviewer_connection(Some(id)).expect("the reviewer's");
  }

  fixture
    .workspace
    .adopt_project_keys(repo.path(), &fixture.vault)
    .expect("adopted");

  let servers = fixture.workspace.decision_servers().expect("read");
  let [server] = servers.as_slice() else {
    panic!("one server: {servers:?}");
  };
  assert_eq!(server.id(), id, "its own id");
  let own = SecretId::from_uuid(id.as_uuid());
  assert_eq!(server.key(), Some(own));
  assert_eq!(
    fixture
      .vault
      .read(&VaultKey::api_account(ApiAccountId::from_uuid(id.as_uuid())))
      .expect("the server's entry")
      .expose(),
    KEY
  );
  let db = ProjectDb::open(repo.path()).expect("database");
  let copy = db.decision_connection(id).expect("read").expect("the project's row");
  assert_eq!(copy.key(), Some(own), "the copy names the server's id as its key's");
  assert_eq!(
    db.decision_reviewer_connection().expect("read"),
    Some(id),
    "the reviewer stays"
  );
}
