//! A project's connection on one of the person's API accounts (`docs/design/agent-accounts.md`, "Agents run on
//! accounts"): made when the account is first used in the project, a copy of the account that holds the account's id
//! as its key's and no secret of the project, found again afterwards, and kept in step when the account changes. No
//! test starts a real CLI: the program set for the CLI is a file that is not there.
//!
//! `cargo test -p orchester-workspace --test api_runs_on api_runs_on`

use std::{collections::HashMap, fs, path::PathBuf};

use orchester_core::{
  account::{AccountCli, ApiSetup},
  connection::{AgentConnection, AgentProvider, BaseUrl, EndpointPreset, NativeModels},
  id::{ApiAccountId, ProjectId, SecretId},
  name::Name,
  project::{Project, ProjectKey},
  resource::ModelId,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  store::{ConnectionStore, SecretStore},
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  accounts::RunsOnError, connections::data::agent_connections, project_setup::NewProject, workspace::Workspace,
};
use parking_lot::Mutex;
use tempfile::TempDir;

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

fn models(ids: &[&str]) -> Vec<ModelId> {
  ids.iter().map(|id| ModelId::new(id).expect("a model id")).collect()
}

fn on_claude(base_url: &str, served: &[&str]) -> ApiSetup {
  ApiSetup::cli(
    AccountCli::ClaudeCode,
    BaseUrl::new(base_url).expect("a base URL"),
    models(served),
  )
  .expect("a setup")
}

/// A local project with no connection, and the program of Claude Code set to a file that is not there.
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
  workspace
    .set_agent_program(AccountCli::ClaudeCode, &data.path().join("no-claude-here.exe"))
    .expect("set");
  Fixture {
    _data: data,
    repo,
    workspace,
    vault: MemoryVault::default(),
  }
}

fn deepseek(fixture: &Fixture) -> ApiAccountId {
  fixture
    .workspace
    .add_api_account(
      &fixture.vault,
      name("DeepSeek"),
      on_claude("https://api.deepseek.com/anthropic", &["deepseek-chat"]),
      &SecretValue::new("sk-typed-by-the-test".to_owned()).expect("a key"),
    )
    .expect("added")
    .id()
}

fn connections(fixture: &Fixture) -> Vec<AgentConnection> {
  agent_connections(
    ProjectDb::open(fixture.repo.path())
      .expect("open")
      .connections()
      .expect("read"),
  )
}

fn nothing_installed(_: AccountCli) -> Option<PathBuf> {
  None
}

#[test]
fn api_runs_on_an_account_gets_a_connection_that_is_its_copy_and_holds_no_secret() {
  let fixture = fixture();
  let account = deepseek(&fixture);

  let made = fixture
    .workspace
    .connect_api_account_with(fixture.repo.path(), account, nothing_installed)
    .expect("connected");

  let listed = connections(&fixture);
  assert_eq!(listed.len(), 1);
  let connection = &listed[0];
  assert_eq!(connection.id(), made);
  assert_eq!(connection.name().as_str(), "Claude Code · DeepSeek");
  assert_eq!(connection.provider(), AgentProvider::ClaudeCodeCli);
  let provider = connection.api_provider().expect("on the API provider");
  assert_eq!(provider.base_url().as_str(), "https://api.deepseek.com/anthropic");
  assert_eq!(
    provider.key(),
    SecretId::from_uuid(account.as_uuid()),
    "the key's id is the account's, which the runner reads from the account's keychain entry"
  );
  assert_eq!(
    provider.model_ids().map(ModelId::as_str).collect::<Vec<_>>(),
    ["deepseek-chat"]
  );
  let db = ProjectDb::open(fixture.repo.path()).expect("open");
  assert!(db.secrets().expect("secrets").is_empty(), "the project holds no secret");

  let again = fixture
    .workspace
    .connect_api_account_with(fixture.repo.path(), account, nothing_installed)
    .expect("there already");
  assert_eq!(again, made);
  assert_eq!(connections(&fixture).len(), 1);
}

#[test]
fn api_runs_on_a_connection_follows_what_the_account_serves_when_the_project_opens() {
  let fixture = fixture();
  let account = deepseek(&fixture);
  let made = fixture
    .workspace
    .connect_api_account_with(fixture.repo.path(), account, nothing_installed)
    .expect("connected");

  fixture
    .workspace
    .change_api_account(
      account,
      on_claude("https://api.deepseek.com/v2", &["deepseek-chat", "deepseek-reasoner"]),
    )
    .expect("changed in Settings");
  fixture.workspace.open(fixture.repo.path()).expect("opened");

  let listed = connections(&fixture);
  assert_eq!(listed.len(), 1, "the same connection, not another");
  assert_eq!(listed[0].id(), made);
  let provider = listed[0].api_provider().expect("on the API provider");
  assert_eq!(provider.base_url().as_str(), "https://api.deepseek.com/v2");
  assert_eq!(
    provider.model_ids().map(ModelId::as_str).collect::<Vec<_>>(),
    ["deepseek-chat", "deepseek-reasoner"]
  );
  assert_eq!(provider.key(), SecretId::from_uuid(account.as_uuid()));
}

#[test]
fn api_runs_on_an_account_of_orchesters_own_agent_is_refused_and_makes_nothing() {
  let fixture = fixture();
  let native = fixture
    .workspace
    .add_api_account(
      &fixture.vault,
      name("Anthropic"),
      ApiSetup::native(
        AgentProvider::AnthropicApi,
        EndpointPreset::Official,
        BaseUrl::new("https://api.anthropic.com").expect("a base URL"),
        NativeModels::default(),
        false,
      )
      .expect("a setup"),
      &SecretValue::new("sk-typed-by-the-test".to_owned()).expect("a key"),
    )
    .expect("added")
    .id();

  let refused = fixture
    .workspace
    .connect_api_account_with(fixture.repo.path(), native, nothing_installed);

  assert!(matches!(refused, Err(RunsOnError::NotForCli)), "{refused:?}");
  assert!(connections(&fixture).is_empty());
}

#[test]
fn api_runs_on_needs_the_program_of_the_cli_to_make_the_connection() {
  let fixture = fixture();
  let account = deepseek(&fixture);
  fixture
    .workspace
    .clear_agent_program(AccountCli::ClaudeCode)
    .expect("cleared");

  let refused = fixture
    .workspace
    .connect_api_account_with(fixture.repo.path(), account, nothing_installed);

  assert!(
    matches!(refused, Err(RunsOnError::CliMissing(AccountCli::ClaudeCode))),
    "{refused:?}"
  );
  assert!(connections(&fixture).is_empty());
}
