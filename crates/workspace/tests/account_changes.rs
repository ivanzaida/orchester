//! What reads the accounts again is told of a change once the project connections that follow it moved: after an
//! account's new folder reached them, and also when moving them failed part way.

mod account_places;

use std::{fs, sync::Arc, thread, time::Duration};

use orchester_core::{
  account::{AccountCli, AgentAccount, MoveTo},
  connection::{AgentConnection, AgentProvider, ConnectionAccount},
  id::{ConnectionId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  accounts::{NewAccount, NewFolder, trash::FolderTrash},
  project_setup::NewProject,
  workspace::Workspace,
};
use tempfile::TempDir;

fn name(text: &str) -> Name {
  Name::new(text).expect("a name")
}

fn workspace(data: &TempDir) -> Workspace {
  let db = AppDb::open(&data.path().join("orchester.db")).expect("the app database");
  let workspace = Workspace::new(db).with_account_root(&data.path().join("accounts"));
  workspace.use_trash(Arc::new(FolderTrash::new(&data.path().join("trash"))));
  workspace
}

fn add(workspace: &Workspace, account: &str) -> AgentAccount {
  workspace
    .add_agent_account(NewAccount {
      cli: AccountCli::Codex,
      name: name(account),
      folder: NewFolder::Create,
    })
    .expect("added")
}

/// A listed project with a Codex connection on `account`.
fn project_on(workspace: &Workspace, account: &AgentAccount) -> TempDir {
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
  ProjectDb::open(repo.path())
    .expect("open")
    .create_connection(&connection.into_connection())
    .expect("created");
  repo
}

/// Holds the write lock of the project's database, as another writer would.
fn hold_writes(repo: &TempDir) -> rusqlite::Connection {
  let held = rusqlite::Connection::open(repo.path().join(".orchester").join("orchester.db")).expect("the database");
  held.execute_batch("BEGIN IMMEDIATE").expect("the write lock");
  held
}

#[test]
fn account_changes_tell_of_a_new_folder_once_the_connections_followed_it() {
  let data = tempfile::tempdir().expect("temporary directory");
  let workspace = workspace(&data);
  let client = add(&workspace, "Client");
  let repo = project_on(&workspace, &client);
  fs::remove_dir_all(client.folder().path().expect("a folder")).expect("the folder goes");
  let place = account_places::in_credential_place();
  let chosen = place.path().join("codex-client-elsewhere");
  fs::create_dir_all(&chosen).expect("the person's folder");
  let mut changes = workspace.account_changes();
  changes.borrow_and_update();
  let held = hold_writes(&repo);
  let choosing = {
    let workspace = workspace.clone();
    thread::spawn(move || workspace.choose_agent_account_folder(client.id(), &chosen))
  };
  // The connection cannot follow while the project's database is held.
  thread::sleep(Duration::from_secs(1));
  assert!(
    !changes.has_changed().expect("the workspace lives"),
    "not told while the connection is still on the old folder"
  );
  held.execute_batch("ROLLBACK").expect("released");
  let chosen = choosing.join().expect("the choice ends").expect("chosen");
  assert!(
    changes.has_changed().expect("the workspace lives"),
    "told once it followed"
  );
  let stored = workspace
    .agent_account(chosen.id())
    .expect("read")
    .expect("still listed");
  assert_eq!(stored.folder(), chosen.folder(), "the new folder is the account's");
}

#[test]
fn account_changes_tell_of_a_removal_whose_connections_could_not_all_move() {
  let data = tempfile::tempdir().expect("temporary directory");
  let workspace = workspace(&data);
  let (client, freelance) = (add(&workspace, "Client"), add(&workspace, "Freelance"));
  let repo = project_on(&workspace, &client);
  let mut changes = workspace.account_changes();
  changes.borrow_and_update();
  // The connection's move gives up after three seconds instead of a minute while the database is held;
  // process-wide, and the other test of this binary lets go of it after one.
  orchester_store::set_lock_budget_for_test(Duration::from_secs(3));
  let held = hold_writes(&repo);
  let refused = workspace.remove_agent_account(client.id(), MoveTo::Account(freelance.id()), false);
  held.execute_batch("ROLLBACK").expect("released");
  assert!(refused.is_err(), "the connection could not move: {refused:?}");
  assert!(
    changes.has_changed().expect("the workspace lives"),
    "what shows the connections reads them again"
  );
}
