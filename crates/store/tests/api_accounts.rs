//! The application database keeps the API accounts and the program of each agent CLI
//! (`docs/design/agent-accounts.md`, "Agents run on accounts").

use std::path::Path;

use orchester_core::{
  account::{AccountCli, ApiAccount, ApiSetup},
  connection::{AgentProvider, BaseUrl, CheckResult, ConnectionCheck, EndpointPreset, NativeModels},
  id::ApiAccountId,
  name::Name,
  resource::ModelId,
  time::Timestamp,
};
use orchester_store::{AppDb, StoreError};

fn name(text: &str) -> Name {
  Name::new(text).expect("a name")
}

fn url(text: &str) -> BaseUrl {
  BaseUrl::new(text).expect("a base URL")
}

fn on_cli(cli: AccountCli, called: &str) -> ApiAccount {
  let models = vec![ModelId::new("deepseek-chat").expect("a model id")];
  let setup = ApiSetup::cli(cli, url("https://api.deepseek.com/anthropic"), models).expect("a setup");
  ApiAccount::new(
    ApiAccountId::generate(),
    name(called),
    setup,
    Timestamp::from_unix_millis(10),
  )
}

fn native(called: &str) -> ApiAccount {
  let setup = ApiSetup::native(
    AgentProvider::AnthropicApi,
    EndpointPreset::Official,
    url("https://api.anthropic.com"),
    NativeModels::default(),
    true,
  )
  .expect("a setup");
  ApiAccount::new(
    ApiAccountId::generate(),
    name(called),
    setup,
    Timestamp::from_unix_millis(20),
  )
}

fn open(dir: &Path) -> AppDb {
  AppDb::open(&dir.join("orchester.db")).expect("open")
}

#[test]
fn api_accounts_an_older_database_gains_the_tables_empty() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  drop(AppDb::create_at_version(&path, 33).expect("version 33"));
  let db = AppDb::open(&path).expect("migrated");
  assert_eq!(db.api_accounts().expect("accounts"), Vec::new());
  assert_eq!(db.agent_program(AccountCli::Codex).expect("program"), None);
}

#[test]
fn api_accounts_are_kept_with_their_setup_and_check() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = open(dir.path());
  let deepseek = on_cli(AccountCli::ClaudeCode, "DeepSeek");
  let mut anthropic = native("Team key");
  db.add_api_account(&deepseek).expect("added");
  db.add_api_account(&anthropic).expect("added");

  anthropic.rename(name("Billing key"));
  anthropic
    .record_check(ConnectionCheck {
      at: Timestamp::from_unix_millis(30),
      result: CheckResult::Api {
        balance: Some("$12.40".to_owned()),
      },
    })
    .expect("an API check");
  db.save_api_account(&anthropic).expect("saved");

  let reopened = open(dir.path());
  assert_eq!(
    reopened.api_accounts().expect("accounts"),
    vec![anthropic.clone(), deepseek.clone()],
    "by kind of agent, as stored"
  );
  assert_eq!(reopened.api_account(deepseek.id()).expect("read"), Some(deepseek));
}

#[test]
fn api_accounts_a_name_is_taken_once_per_kind_of_agent() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = open(dir.path());
  db.add_api_account(&on_cli(AccountCli::ClaudeCode, "DeepSeek"))
    .expect("added");
  db.add_api_account(&on_cli(AccountCli::Codex, "DeepSeek"))
    .expect("another kind of agent takes the name too");

  let again = db.add_api_account(&on_cli(AccountCli::ClaudeCode, "deepseek"));
  assert!(
    matches!(&again, Err(StoreError::DuplicateAccountName(taken)) if taken == "deepseek"),
    "{again:?}"
  );

  let mut gateway = on_cli(AccountCli::ClaudeCode, "Gateway");
  db.add_api_account(&gateway).expect("added");
  gateway.rename(name("DeepSeek"));
  let renamed = db.save_api_account(&gateway);
  assert!(
    matches!(renamed, Err(StoreError::DuplicateAccountName(_))),
    "{renamed:?}"
  );
}

#[test]
fn api_accounts_a_removed_one_leaves_the_list_and_frees_its_name() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = open(dir.path());
  let deepseek = on_cli(AccountCli::ClaudeCode, "DeepSeek");
  db.add_api_account(&deepseek).expect("added");
  db.remove_api_account(deepseek.id(), Timestamp::from_unix_millis(40))
    .expect("removed");

  assert_eq!(db.api_accounts().expect("accounts"), Vec::new());
  assert_eq!(
    db.api_account(deepseek.id()).expect("read"),
    Some(deepseek.clone()),
    "a finished run still names it"
  );
  let saved = db.save_api_account(&deepseek);
  assert!(
    matches!(saved, Err(StoreError::ApiAccountNotFound(id)) if id == deepseek.id()),
    "{saved:?}"
  );
  let twice = db.remove_api_account(deepseek.id(), Timestamp::from_unix_millis(50));
  assert!(matches!(twice, Err(StoreError::ApiAccountNotFound(_))), "{twice:?}");
  db.add_api_account(&on_cli(AccountCli::ClaudeCode, "DeepSeek"))
    .expect("the name is free again");
}

#[test]
fn api_accounts_a_setup_of_another_kind_of_agent_is_not_read() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  let deepseek = on_cli(AccountCli::ClaudeCode, "DeepSeek");
  let mut db = AppDb::open(&path).expect("open");
  db.add_api_account(&deepseek).expect("added");
  drop(db);

  let raw = rusqlite::Connection::open(&path).expect("raw");
  raw
    .execute("UPDATE api_accounts SET agent = 'anthropic_api'", [])
    .expect("edited");
  drop(raw);

  let read = AppDb::open(&path).expect("open").api_accounts();
  assert!(matches!(read, Err(StoreError::Corrupt { .. })), "{read:?}");
}

#[test]
fn agent_programs_are_set_replaced_and_cleared_per_cli() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = open(dir.path());
  let first = dir.path().join("claude.exe");
  let second = dir.path().join("bin").join("claude.exe");

  db.set_agent_program(AccountCli::ClaudeCode, &first, Timestamp::from_unix_millis(1))
    .expect("set");
  db.set_agent_program(AccountCli::ClaudeCode, &second, Timestamp::from_unix_millis(2))
    .expect("replaced");
  assert_eq!(db.agent_program(AccountCli::ClaudeCode).expect("program"), Some(second));
  assert_eq!(db.agent_program(AccountCli::Codex).expect("program"), None);

  assert!(db.clear_agent_program(AccountCli::ClaudeCode).expect("cleared"));
  assert!(!db.clear_agent_program(AccountCli::ClaudeCode).expect("nothing left"));
  assert_eq!(db.agent_program(AccountCli::ClaudeCode).expect("program"), None);
}
