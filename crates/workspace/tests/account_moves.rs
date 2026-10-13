//! Accounts that move and folders that go: a brought folder is checked beside every CLI's accounts; a removal never
//! bins a folder that holds another account's; connections that follow an account or its folder are checked again
//! there; and a trash that fails says the account was removed and where its folder stays.

use std::{fs, path::PathBuf, sync::Arc};

use orchester_core::{
  account::{AccountCli, AccountFolder, AgentAccount, FolderProblem, MoveTo},
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
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  accounts::{AccountsError, NewAccount, NewFolder, trash::FolderTrash},
  project_setup::NewProject,
  workspace::Workspace,
};
use tempfile::TempDir;

struct Setup {
  data: TempDir,
  workspace: Workspace,
}

fn setup() -> Setup {
  let data = tempfile::tempdir().expect("temporary directory");
  let db = AppDb::open(&data.path().join("orchester.db")).expect("the app database");
  let workspace = Workspace::new(db).with_account_root(&data.path().join("accounts"));
  workspace.use_trash(Arc::new(FolderTrash::new(&data.path().join("trash"))));
  Setup { data, workspace }
}

fn name(text: &str) -> Name {
  Name::new(text).expect("a name")
}

fn add(workspace: &Workspace, cli: AccountCli, account: &str) -> AgentAccount {
  workspace
    .add_agent_account(NewAccount {
      cli,
      name: name(account),
      folder: NewFolder::Create,
    })
    .expect("added")
}

/// A listed project with a Codex connection on `account`, checked and signed in.
fn project_on(workspace: &Workspace, account: &AgentAccount) -> (TempDir, ConnectionId) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    name("Kontur"),
  );
  workspace
    .create(NewProject {
      project,
      repo_root: repo.path().to_path_buf(),
    })
    .expect("created");
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name("Codex"),
    AgentProvider::CodexCli,
    repo.path().join("missing-codex.exe"),
    Timestamp::now(),
  )
  .expect("absolute");
  connection
    .set_account(ConnectionAccount::of(account))
    .expect("a Codex account");
  connection
    .record_check(ConnectionCheck {
      at: Timestamp::from_unix_millis(1),
      result: CheckResult::Available {
        version: "0.159.2".to_owned(),
        sign_in: SignIn::SignedIn,
        catalog: ModelCatalog::new(CatalogSource::Provider, Vec::new()),
      },
    })
    .expect("its kind");
  let id = connection.id();
  ProjectDb::open(repo.path())
    .expect("open")
    .create_connection(&connection.into_connection())
    .expect("created");
  (repo, id)
}

fn checked_at(repo: &std::path::Path, id: ConnectionId) -> Option<Timestamp> {
  let connection = ProjectDb::open(repo)
    .expect("open")
    .connection(id)
    .expect("read")
    .expect("stored");
  AgentConnection::try_from(connection)
    .expect("an agent connection")
    .last_check()
    .map(|check| check.at)
}

#[test]
fn account_moves_a_brought_folder_is_checked_beside_every_clis_accounts() {
  let Setup { data, workspace } = setup();
  let work = brought_as_before(
    data.path(),
    AccountCli::ClaudeCode,
    "Work",
    data.path().join("claude-work"),
  );
  let inside = data.path().join("claude-work").join("codex");
  let problem = workspace.brought_folder_problem(&inside).expect("checked");
  assert_eq!(
    problem,
    Some(FolderProblem::Taken(work.id())),
    "a Codex folder inside Claude's Work"
  );
}

#[test]
fn account_moves_a_removal_never_bins_a_folder_that_holds_another_account() {
  let Setup { data, workspace } = setup();
  let work = add(&workspace, AccountCli::ClaudeCode, "Work");
  let folder = work.folder().path().expect("a folder").to_path_buf();
  // Added past the folder rules, as an older Orchester could: a Codex account inside Claude's Work.
  let inner = folder.join("codex");
  fs::create_dir_all(&inner).expect("the folder");
  let codex = AgentAccount::new(
    AgentAccountId::generate(),
    AccountCli::Codex,
    name("Inner"),
    AccountFolder::Brought(inner.clone()),
    Timestamp::now(),
  )
  .expect("an account");
  AppDb::open(&data.path().join("orchester.db"))
    .expect("the app database")
    .add_agent_account(&codex)
    .expect("added");
  let refused = workspace.remove_agent_account(work.id(), MoveTo::NoAccount, true);
  assert!(
    matches!(refused, Err(AccountsError::HoldsAnotherAccount)),
    "{refused:?}"
  );
  assert!(inner.is_dir(), "the other account's folder stays");
}

#[test]
fn account_moves_connections_that_follow_a_folder_are_checked_again_there() {
  let Setup { data, workspace } = setup();
  let client = add(&workspace, AccountCli::Codex, "Client");
  let (repo, connection) = project_on(&workspace, &client);
  let before = checked_at(repo.path(), connection);
  // The account's folder moved while the project was out of reach: it follows when it opens.
  let mut moved = client;
  moved
    .move_to_folder(data.path().join("elsewhere"))
    .expect("an absolute folder");
  AppDb::open(&data.path().join("orchester.db"))
    .expect("the app database")
    .save_agent_account_folder(&moved)
    .expect("saved");
  workspace.open(repo.path()).expect("opened");
  let stored = AgentConnection::try_from(
    ProjectDb::open(repo.path())
      .expect("open")
      .connection(connection)
      .expect("read")
      .expect("stored"),
  )
  .expect("an agent connection");
  assert_eq!(stored.account(), &ConnectionAccount::of(&moved));
  assert!(
    stored.last_check().map(|check| check.at) > before,
    "checked again in the new folder, which may be signed in to someone else"
  );
  assert!(
    !stored.status().is_usable(),
    "the old folder's \"signed in\" does not carry over"
  );
}

#[test]
fn account_moves_choosing_a_folder_checks_the_accounts_connections_there() {
  let Setup { data, workspace } = setup();
  let client = add(&workspace, AccountCli::Codex, "Client");
  let (repo, connection) = project_on(&workspace, &client);
  fs::remove_dir_all(client.folder().path().expect("a folder")).expect("the folder goes");
  let chosen = data.path().join("codex-client-elsewhere");
  fs::create_dir_all(&chosen).expect("the person's folder");
  workspace
    .choose_agent_account_folder(client.id(), &chosen)
    .expect("chosen");
  let stored = AgentConnection::try_from(
    ProjectDb::open(repo.path())
      .expect("open")
      .connection(connection)
      .expect("read")
      .expect("stored"),
  )
  .expect("an agent connection");
  assert!(
    stored
      .last_check()
      .is_some_and(|check| check.at > Timestamp::from_unix_millis(1)),
    "checked again in the chosen folder"
  );
  assert!(!stored.status().is_usable(), "not the old folder's sign-in");
}

#[test]
fn account_moves_connections_moved_to_another_account_are_checked_there() {
  let Setup { workspace, .. } = setup();
  let (client, freelance) = (
    add(&workspace, AccountCli::Codex, "Client"),
    add(&workspace, AccountCli::Codex, "Freelance"),
  );
  let (repo, connection) = project_on(&workspace, &client);
  workspace
    .remove_agent_account(client.id(), MoveTo::Account(freelance.id()), false)
    .expect("removed");
  let checked = checked_at(repo.path(), connection);
  assert!(
    checked.is_some_and(|at| at > Timestamp::from_unix_millis(1)),
    "checked again on Freelance, not left unchecked"
  );
}

#[test]
fn account_moves_a_trash_that_fails_says_the_account_was_removed() {
  let Setup { data, workspace } = setup();
  let blocked = data.path().join("not-a-folder");
  fs::write(&blocked, "a file where the trash folder would be").expect("a file");
  workspace.use_trash(Arc::new(FolderTrash::new(&blocked.join("trash"))));
  let work = add(&workspace, AccountCli::Codex, "Work");
  let folder: PathBuf = work.folder().path().expect("a folder").to_path_buf();
  let refused = workspace.remove_agent_account(work.id(), MoveTo::NoAccount, true);
  match refused {
    Err(AccountsError::TrashFailed { folder: stays, .. }) => assert_eq!(stays, folder),
    other => panic!("a partial success, not {other:?}"),
  }
  assert!(folder.is_dir(), "the folder stays where it is");
  assert!(
    workspace
      .agent_accounts()
      .expect("listed")
      .iter()
      .all(|account| account.id() != work.id()),
    "the account is removed"
  );
}

/// An account brought past the folder rules, as an older Orchester could, stored as is.
fn brought_as_before(data: &std::path::Path, cli: AccountCli, account: &str, folder: PathBuf) -> AgentAccount {
  fs::create_dir_all(&folder).expect("the folder");
  let brought = AgentAccount::new(
    AgentAccountId::generate(),
    cli,
    name(account),
    AccountFolder::Brought(folder),
    Timestamp::now(),
  )
  .expect("an account");
  AppDb::open(&data.join("orchester.db"))
    .expect("the app database")
    .add_agent_account(&brought)
    .expect("added");
  brought
}

#[test]
fn account_moves_the_accounts_folder_is_never_brought_nor_what_holds_or_lies_in_it() {
  let Setup { data, workspace } = setup();
  let root = data.path().join("accounts");
  fs::create_dir_all(root.join("inner")).expect("the accounts' folder");
  for folder in [root.clone(), root.join("inner"), data.path().to_path_buf()] {
    assert_eq!(
      workspace.brought_folder_problem(&folder).expect("checked"),
      Some(FolderProblem::AccountsFolder),
      "{}",
      folder.display()
    );
  }
}

#[test]
fn account_moves_a_new_folder_never_lands_in_a_brought_one() {
  let Setup { data, workspace } = setup();
  let old = brought_as_before(data.path(), AccountCli::Codex, "Old", data.path().join("accounts"));
  let refused = workspace.add_agent_account(NewAccount {
    cli: AccountCli::Codex,
    name: name("Work"),
    folder: NewFolder::Create,
  });
  assert!(
    matches!(refused, Err(AccountsError::Folder(FolderProblem::Taken(id))) if id == old.id()),
    "{refused:?}"
  );
}

#[test]
fn account_moves_a_removal_never_bins_a_folder_inside_another_accounts() {
  let Setup { data, workspace } = setup();
  let work = add(&workspace, AccountCli::Codex, "Work");
  // Brought past the rules: the whole accounts' folder, Work's inside it.
  brought_as_before(
    data.path(),
    AccountCli::ClaudeCode,
    "Everything",
    data.path().join("accounts"),
  );
  let folder = work.folder().path().expect("a folder").to_path_buf();
  let refused = workspace.remove_agent_account(work.id(), MoveTo::NoAccount, true);
  assert!(
    matches!(refused, Err(AccountsError::HoldsAnotherAccount)),
    "{refused:?}"
  );
  assert!(folder.is_dir(), "the folder stays, part of Everything's");
}
