//! App-wide agent accounts: adding with a new or brought folder, the folder rules, renaming, removing with the
//! connections of every listed project moved, and a project reached later following the removal.

use std::{fs, path::Path};

use orchester_core::{
  account::{AccountCli, FolderProblem, MoveTo},
  connection::{AgentConnection, AgentProvider, ConnectionAccount},
  id::{ConnectionId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  store::ConnectionStore,
  time::Timestamp,
};
use orchester_store::{AppDb, ProjectDb};
use orchester_workspace::{
  accounts::{AccountsError, NewAccount, NewFolder},
  connections::data::set_account_in,
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
  Setup { data, workspace }
}

fn name(text: &str) -> Name {
  Name::new(text).expect("a name")
}

/// A listed project with one Claude Code connection.
fn project(workspace: &Workspace, key: &str) -> (TempDir, ConnectionId) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new(key).expect("valid key"),
    name("Kontur"),
  );
  workspace
    .create(NewProject {
      project,
      repo_root: repo.path().to_path_buf(),
    })
    .expect("created");
  let connection = AgentConnection::new(
    ConnectionId::generate(),
    name("Claude"),
    AgentProvider::ClaudeCodeCli,
    repo.path().join("missing-claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute");
  ProjectDb::open(repo.path())
    .expect("open")
    .create_connection(&connection)
    .expect("created");
  (repo, connection.id())
}

fn account_of(repo: &Path, connection: ConnectionId) -> ConnectionAccount {
  ProjectDb::open(repo)
    .expect("open")
    .connection(connection)
    .expect("read")
    .expect("stored")
    .account()
    .clone()
}

#[test]
fn accounts_a_new_account_gets_a_folder_orchester_creates() {
  let Setup { data, workspace } = setup();
  let planned = workspace.planned_folder(AccountCli::Codex, "Client").expect("a root");
  assert_eq!(planned, data.path().join("accounts").join("codex-client"));
  let added = workspace
    .add_agent_account(NewAccount {
      cli: AccountCli::Codex,
      name: name("Client"),
      folder: NewFolder::Create,
    })
    .expect("added");
  assert!(planned.is_dir());
  assert_eq!(added.folder().path(), Some(planned.as_path()));
  assert_eq!(
    workspace.planned_folder(AccountCli::Codex, "Client"),
    Some(data.path().join("accounts").join("codex-client-2"))
  );
  let listed = workspace.agent_accounts().expect("listed");
  assert_eq!(listed.len(), 3, "two Defaults and Client");
  let again = workspace.add_agent_account(NewAccount {
    cli: AccountCli::Codex,
    name: name("client"),
    folder: NewFolder::Create,
  });
  assert!(matches!(
    again,
    Err(AccountsError::Store(orchester_store::StoreError::DuplicateAccountName(
      _
    )))
  ));
}

#[test]
fn accounts_a_brought_folder_inside_a_repository_is_refused() {
  let Setup { workspace, .. } = setup();
  let (repo, _) = project(&workspace, "KON");
  let inside = repo.path().join(".claude-ci");
  fs::create_dir(&inside).expect("a folder");
  let problem = workspace
    .brought_folder_problem(AccountCli::ClaudeCode, &inside)
    .expect("checked");
  assert_eq!(
    problem,
    Some(FolderProblem::InsideRepository(repo.path().to_path_buf()))
  );
  let refused = workspace.add_agent_account(NewAccount {
    cli: AccountCli::ClaudeCode,
    name: name("CI"),
    folder: NewFolder::Bring(inside),
  });
  assert!(matches!(
    refused,
    Err(AccountsError::Folder(FolderProblem::InsideRepository(_)))
  ));
}

#[test]
fn accounts_renaming_changes_only_the_name() {
  let Setup { workspace, .. } = setup();
  let added = workspace
    .add_agent_account(NewAccount {
      cli: AccountCli::Codex,
      name: name("Client"),
      folder: NewFolder::Create,
    })
    .expect("added");
  let renamed = workspace
    .rename_agent_account(added.id(), name("Kontur team"))
    .expect("renamed");
  assert_eq!(renamed.name(), Some(&name("Kontur team")));
  assert_eq!(renamed.folder(), added.folder());
  let default = workspace
    .rename_agent_account(AccountCli::Codex.default_account(), name("Mine"))
    .expect("Default can be renamed");
  assert!(default.is_default());
}

#[test]
fn accounts_a_connection_chooses_its_account_and_removal_moves_it() {
  let Setup { workspace, .. } = setup();
  let work = workspace
    .add_agent_account(NewAccount {
      cli: AccountCli::ClaudeCode,
      name: name("Work"),
      folder: NewFolder::Create,
    })
    .expect("added");
  let (first, first_connection) = project(&workspace, "KON");
  let (second, second_connection) = project(&workspace, "ATL");
  set_account_in(first.path(), first_connection, &work).expect("chosen");
  set_account_in(second.path(), second_connection, &work).expect("chosen");
  assert_eq!(account_of(first.path(), first_connection), ConnectionAccount::of(&work));

  let uses = workspace.agent_account_uses(work.id()).expect("uses");
  assert_eq!(uses.connections.len(), 2);
  assert_eq!(uses.project_count(), 2);
  assert!(uses.running.is_empty());

  let refused = workspace.remove_agent_account(AccountCli::ClaudeCode.default_account(), MoveTo::NoAccount, false);
  assert!(matches!(refused, Err(AccountsError::Removal(_))));

  workspace
    .remove_agent_account(
      work.id(),
      MoveTo::Account(AccountCli::ClaudeCode.default_account()),
      false,
    )
    .expect("removed");
  assert_eq!(account_of(first.path(), first_connection), ConnectionAccount::Default);
  assert_eq!(account_of(second.path(), second_connection), ConnectionAccount::Default);
  assert!(
    work.folder().path().expect("a folder").is_dir(),
    "the folder stays unless asked"
  );
  assert!(
    workspace
      .agent_accounts()
      .expect("listed")
      .iter()
      .all(|account| account.id() != work.id())
  );
  assert!(
    workspace.agent_account(work.id()).expect("read").is_some(),
    "runs still name it"
  );
}

#[test]
fn accounts_a_project_out_of_reach_at_removal_follows_it_when_it_opens() {
  let Setup { workspace, .. } = setup();
  let work = workspace
    .add_agent_account(NewAccount {
      cli: AccountCli::ClaudeCode,
      name: name("Work"),
      folder: NewFolder::Create,
    })
    .expect("added");
  let (repo, connection) = project(&workspace, "KON");
  set_account_in(repo.path(), connection, &work).expect("chosen");
  let away = repo.path().with_extension("away");
  fs::rename(repo.path(), &away).expect("moved away");
  workspace
    .remove_agent_account(work.id(), MoveTo::NoAccount, false)
    .expect("removed");
  fs::rename(&away, repo.path()).expect("moved back");
  assert_eq!(account_of(repo.path(), connection), ConnectionAccount::of(&work));
  workspace.open(repo.path()).expect("opened");
  assert_eq!(account_of(repo.path(), connection), ConnectionAccount::Unassigned);
}
