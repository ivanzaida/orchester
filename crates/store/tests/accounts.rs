use std::{fs, path::PathBuf};

use orchester_core::{
  account::{AccountCheck, AccountCheckResult, AccountCli, AccountFolder, AgentAccount, CheckedProgram},
  connection::{
    AgentConnection, AgentProvider, CatalogSource, CheckResult, ConnectionAccount, ConnectionCheck, ModelCatalog,
    SignIn,
  },
  id::{AgentAccountId, ConnectionId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb, StoreError};

fn account(cli: AccountCli, name: &str, folder: PathBuf) -> AgentAccount {
  AgentAccount::new(
    AgentAccountId::generate(),
    cli,
    Name::new(name).expect("a name"),
    AccountFolder::Created(folder),
    Timestamp::from_unix_millis(10),
  )
  .expect("an account")
}

#[test]
fn accounts_start_with_a_default_per_cli() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  let accounts = db.agent_accounts().expect("accounts");
  assert_eq!(
    accounts,
    vec![
      AgentAccount::default_of(AccountCli::ClaudeCode),
      AgentAccount::default_of(AccountCli::Codex)
    ]
  );
}

#[test]
fn accounts_an_older_database_gains_the_defaults() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let path = dir.path().join("orchester.db");
  drop(AppDb::create_at_version(&path, 8).expect("version 8"));
  let db = AppDb::open(&path).expect("migrated");
  assert_eq!(db.agent_accounts().expect("accounts").len(), 2);
}

#[test]
fn accounts_are_added_renamed_checked_and_removed_with_their_rows_kept() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  let mut work = account(AccountCli::ClaudeCode, "Work", dir.path().join("claude-work"));
  db.add_agent_account(&work).expect("added");
  work.rename(Name::new("Kontur team").expect("a name"));
  work.record_check(AccountCheck {
    at: Timestamp::from_unix_millis(20),
    result: AccountCheckResult::SignedOut {
      program: CheckedProgram {
        path: dir.path().join("claude.exe"),
        version: "2.1.296".into(),
      },
      was: None,
    },
  });
  db.save_agent_account(&work).expect("saved");
  let listed = db.agent_accounts().expect("accounts");
  assert_eq!(listed.iter().find(|account| account.id() == work.id()), Some(&work));

  db.remove_agent_account(work.id(), None, Timestamp::from_unix_millis(30))
    .expect("removed");
  assert!(
    db.agent_accounts()
      .expect("accounts")
      .iter()
      .all(|account| account.id() != work.id())
  );
  assert_eq!(db.agent_account(work.id()).expect("read"), Some(work.clone()));
  assert_eq!(db.removed_agent_accounts().expect("removed"), vec![(work.id(), None)]);
  assert!(matches!(
    db.remove_agent_account(AccountCli::Codex.default_account(), None, Timestamp::now()),
    Err(StoreError::AgentAccountNotFound(_))
  ));
}

#[test]
fn accounts_names_are_unique_per_cli_and_folders_unique_overall() {
  let dir = tempfile::tempdir().expect("temporary directory");
  let mut db = AppDb::open(&dir.path().join("orchester.db")).expect("open");
  db.add_agent_account(&account(AccountCli::ClaudeCode, "Work", dir.path().join("a")))
    .expect("added");
  db.add_agent_account(&account(AccountCli::Codex, "Work", dir.path().join("b")))
    .expect("the other CLI's Work");
  assert!(matches!(
    db.add_agent_account(&account(AccountCli::ClaudeCode, "work", dir.path().join("c"))),
    Err(StoreError::DuplicateAccountName(_))
  ));
  assert!(matches!(
    db.add_agent_account(&account(AccountCli::ClaudeCode, "Personal", dir.path().join("a"))),
    Err(StoreError::AccountFolderTaken)
  ));
}

#[test]
fn accounts_a_connection_keeps_its_account_and_older_ones_read_as_default() {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("create project");
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Claude").expect("valid name"),
    AgentProvider::ClaudeCodeCli,
    repo.path().join("claude.exe"),
    Timestamp::from_unix_millis(1),
  )
  .expect("absolute");
  db.create_connection(&connection).expect("created");
  assert_eq!(
    db.connection(connection.id()).expect("read").expect("stored").account(),
    &ConnectionAccount::Default
  );
  let account = ConnectionAccount::Account {
    id: AgentAccountId::generate(),
    folder: repo.path().join("claude-work"),
  };
  connection.set_account(account.clone()).expect("an agent CLI");
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::from_unix_millis(2),
      result: CheckResult::Available {
        version: "2.1.296".into(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    })
    .expect("its kind");
  db.save_connection(&connection).expect("saved");
  let stored = db.connection(connection.id()).expect("read").expect("stored");
  assert_eq!(stored.account(), &account);
  assert!(stored.last_check().is_some(), "the check made on the account stays");
}
