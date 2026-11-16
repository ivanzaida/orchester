//! App-wide API accounts (`docs/design/agent-accounts.md`, "Agents run on accounts"): adding one with its key in the
//! keychain, the names it shares with the sign-ins of its CLI, replacing the key, and the program of each agent CLI.

use std::collections::HashMap;

use orchester_core::{
  account::{AccountCli, ApiAccountError, ApiSetup},
  connection::{AgentProvider, BaseUrl, CheckResult, ConnectionCheck, EndpointPreset, NativeModels},
  name::Name,
  resource::ModelId,
  secret::{SecretValue, SecretVault, VaultError, VaultKey},
  time::Timestamp,
};
use orchester_store::{AppDb, StoreError};
use orchester_workspace::{
  accounts::{AccountsError, NewAccount, NewFolder},
  workspace::Workspace,
};
use parking_lot::Mutex;
use tempfile::TempDir;

/// The key a test types, which no error or debug text may hold.
const TYPED: &str = "sk-typed-by-the-test";

#[derive(Default)]
struct MemoryVault(Mutex<HashMap<VaultKey, String>>);

impl MemoryVault {
  fn value(&self, key: &VaultKey) -> Option<String> {
    self.0.lock().get(key).cloned()
  }

  fn len(&self) -> usize {
    self.0.lock().len()
  }
}

impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.0.lock().insert(key.clone(), value.expose().to_owned());
    Ok(())
  }

  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    let value = self.0.lock().get(key).cloned().ok_or(VaultError::NotFound)?;
    SecretValue::new(value).map_err(|_| VaultError::NotFound)
  }

  fn exists(&self, key: &VaultKey) -> Result<bool, VaultError> {
    Ok(self.0.lock().contains_key(key))
  }

  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.0.lock().remove(key);
    Ok(())
  }
}

/// A keychain that refuses every value, as a locked one does.
struct RefusingVault;

impl SecretVault for RefusingVault {
  fn store(&self, _: &VaultKey, _: &SecretValue) -> Result<(), VaultError> {
    Err(VaultError::AccessDenied("refused by the test".to_owned()))
  }

  fn read(&self, _: &VaultKey) -> Result<SecretValue, VaultError> {
    Err(VaultError::NotFound)
  }

  fn delete(&self, _: &VaultKey) -> Result<(), VaultError> {
    Ok(())
  }
}

fn setup() -> (TempDir, Workspace) {
  let data = tempfile::tempdir().expect("temporary directory");
  let db = AppDb::open(&data.path().join("orchester.db")).expect("the app database");
  let workspace = Workspace::new(db).with_account_root(&data.path().join("accounts"));
  (data, workspace)
}

fn name(text: &str) -> Name {
  Name::new(text).expect("a name")
}

fn key(text: &str) -> SecretValue {
  SecretValue::new(text.to_owned()).expect("a key")
}

fn on_claude(base_url: &str) -> ApiSetup {
  ApiSetup::cli(
    AccountCli::ClaudeCode,
    BaseUrl::new(base_url).expect("a base URL"),
    vec![ModelId::new("deepseek-chat").expect("a model id")],
  )
  .expect("a setup")
}

fn anthropic(fallback: bool) -> ApiSetup {
  ApiSetup::native(
    AgentProvider::AnthropicApi,
    EndpointPreset::Official,
    BaseUrl::new("https://api.anthropic.com").expect("a base URL"),
    NativeModels::default(),
    fallback,
  )
  .expect("a setup")
}

#[test]
fn api_account_is_added_with_its_key_in_the_keychain() {
  let (_data, workspace) = setup();
  let vault = MemoryVault::default();
  let changes = workspace.account_changes();
  let before = *changes.borrow();

  let added = workspace
    .add_api_account(
      &vault,
      name("DeepSeek"),
      on_claude("https://api.deepseek.com/anthropic"),
      &key(TYPED),
    )
    .expect("added");

  assert_eq!(workspace.api_accounts().expect("accounts"), vec![added.clone()]);
  assert_eq!(vault.value(&VaultKey::api_account(added.id())).as_deref(), Some(TYPED));
  assert!(workspace.api_account_has_key(&vault, added.id()).expect("looked"));
  assert!(*changes.borrow() > before, "what lists accounts reads them again");
}

#[test]
fn api_account_refused_by_the_database_leaves_no_key() {
  let (_data, workspace) = setup();
  let vault = MemoryVault::default();
  workspace
    .add_api_account(
      &vault,
      name("DeepSeek"),
      on_claude("https://api.deepseek.com/anthropic"),
      &key(TYPED),
    )
    .expect("added");

  let again = workspace.add_api_account(
    &vault,
    name("deepseek"),
    on_claude("https://gateway.example"),
    &key(TYPED),
  );
  assert!(
    matches!(&again, Err(AccountsError::Store(StoreError::DuplicateAccountName(_)))),
    "{again:?}"
  );
  assert!(!format!("{again:?}").contains(TYPED));
  assert_eq!(vault.len(), 1, "only the first account's key is kept");
}

#[test]
fn api_account_a_keychain_that_refuses_adds_nothing() {
  let (_data, workspace) = setup();
  let refused = workspace.add_api_account(&RefusingVault, name("DeepSeek"), anthropic(false), &key(TYPED));
  assert!(
    matches!(&refused, Err(AccountsError::Keychain(VaultError::AccessDenied(_)))),
    "{refused:?}"
  );
  assert!(!format!("{refused:?}").contains(TYPED));
  assert_eq!(workspace.api_accounts().expect("accounts"), Vec::new());
}

#[test]
fn api_account_and_a_sign_in_of_its_cli_never_share_a_name() {
  let (_data, workspace) = setup();
  let vault = MemoryVault::default();
  let work = workspace
    .add_agent_account(NewAccount {
      cli: AccountCli::ClaudeCode,
      name: name("Work"),
      folder: NewFolder::Create,
    })
    .expect("a sign-in");

  let taken = workspace.add_api_account(&vault, name("work"), on_claude("https://gateway.example"), &key(TYPED));
  assert!(
    matches!(taken, Err(AccountsError::Store(StoreError::DuplicateAccountName(_)))),
    "a sign-in of Claude Code goes by the name"
  );
  workspace
    .add_api_account(&vault, name("Work"), anthropic(false), &key(TYPED))
    .expect("another kind of agent takes the name");

  let gateway = workspace
    .add_api_account(
      &vault,
      name("Gateway"),
      on_claude("https://gateway.example"),
      &key(TYPED),
    )
    .expect("added");
  let renamed = workspace.rename_api_account(gateway.id(), name("Work"));
  assert!(
    matches!(renamed, Err(AccountsError::Store(StoreError::DuplicateAccountName(_)))),
    "{renamed:?}"
  );

  let sign_in_renamed = workspace.rename_agent_account(work.id(), name("gateway"));
  assert!(
    matches!(
      sign_in_renamed,
      Err(AccountsError::Store(StoreError::DuplicateAccountName(_)))
    ),
    "an API account of Claude Code goes by the name"
  );
  let sign_in_added = workspace.add_agent_account(NewAccount {
    cli: AccountCli::ClaudeCode,
    name: name("Gateway"),
    folder: NewFolder::Create,
  });
  assert!(
    matches!(
      sign_in_added,
      Err(AccountsError::Store(StoreError::DuplicateAccountName(_)))
    ),
    "{sign_in_added:?}"
  );
}

#[test]
fn api_account_a_replaced_key_drops_the_check_made_with_the_one_before() {
  let (data, workspace) = setup();
  let vault = MemoryVault::default();
  let added = workspace
    .add_api_account(&vault, name("Team key"), anthropic(false), &key(TYPED))
    .expect("added");
  let check = ConnectionCheck {
    at: Timestamp::from_unix_millis(5),
    result: CheckResult::Api { balance: None },
  };
  let mut checked = added.clone();
  checked.record_check(check.clone()).expect("an API check");
  AppDb::open(&data.path().join("orchester.db"))
    .expect("the app database")
    .save_api_account(&checked)
    .expect("the check kept");

  let changed = workspace
    .change_api_account(added.id(), anthropic(true))
    .expect("provider fallback on");
  assert_eq!(changed.setup(), &anthropic(true));
  assert_eq!(changed.last_check(), Some(&check), "the API is the same");

  let replaced = workspace
    .replace_api_account_key(&vault, added.id(), &key("sk-the-next-one"))
    .expect("replaced");
  assert_eq!(replaced.last_check(), None);
  assert_eq!(
    vault.value(&VaultKey::api_account(added.id())).as_deref(),
    Some("sk-the-next-one")
  );

  let other_agent = workspace.change_api_account(added.id(), on_claude("https://gateway.example"));
  assert!(
    matches!(
      other_agent,
      Err(AccountsError::ApiAccount(ApiAccountError::AnotherAgent))
    ),
    "{other_agent:?}"
  );
}

#[test]
fn agent_program_is_an_absolute_path_set_once_for_every_project() {
  let (data, workspace) = setup();
  let program = data.path().join("claude.exe");

  let relative = workspace.set_agent_program(AccountCli::ClaudeCode, std::path::Path::new("claude.exe"));
  assert!(
    matches!(relative, Err(AccountsError::RelativeProgram(_))),
    "{relative:?}"
  );
  assert_eq!(workspace.agent_program(AccountCli::ClaudeCode).expect("program"), None);

  workspace
    .set_agent_program(AccountCli::ClaudeCode, &program)
    .expect("set");
  assert_eq!(
    workspace.agent_program(AccountCli::ClaudeCode).expect("program"),
    Some(program)
  );
  workspace.clear_agent_program(AccountCli::ClaudeCode).expect("cleared");
  assert_eq!(workspace.agent_program(AccountCli::ClaudeCode).expect("program"), None);
}
