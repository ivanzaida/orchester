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
  accounts::{AccountsError, NewAccount, NewFolder, trash::FolderTrash},
  connections::data::set_account_in,
  project_setup::NewProject,
  workspace::Workspace,
};
use tempfile::TempDir;

mod support;

struct Setup {
  data: TempDir,
  workspace: Workspace,
}

fn setup() -> Setup {
  let data = tempfile::tempdir().expect("temporary directory");
  let db = AppDb::open(&data.path().join("orchester.db")).expect("the app database");
  let workspace = Workspace::new(db).with_account_root(&data.path().join("accounts"));
  // Never the person's Recycle Bin: a folder of the test's own.
  workspace.use_trash(std::sync::Arc::new(FolderTrash::new(&data.path().join("trash"))));
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
  assert!(
    !data.path().join("accounts").join("codex-client-2").exists(),
    "a refused account leaves no folder"
  );
}

#[test]
fn accounts_a_brought_folder_inside_a_repository_is_refused() {
  let Setup { workspace, .. } = setup();
  let (repo, _) = project(&workspace, "KON");
  let inside = repo.path().join(".claude-ci");
  fs::create_dir(&inside).expect("a folder");
  let problem = workspace.brought_folder_problem(&inside).expect("checked");
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

/// A connection called `name` on `account` in the fixture's project.
fn connection_on(
  fixture: &mut support::Fixture,
  name_of: &str,
  account: &orchester_core::account::AgentAccount,
) -> ConnectionId {
  let mut connection = AgentConnection::new(
    ConnectionId::generate(),
    name(name_of),
    AgentProvider::ClaudeCodeCli,
    fixture.repo.path().join("claude.exe"),
    Timestamp::now(),
  )
  .expect("absolute");
  connection
    .set_account(ConnectionAccount::of(account))
    .expect("a Claude Code account");
  fixture.db.create_connection(&connection).expect("created");
  connection.id()
}

fn add(workspace: &Workspace, account_name: &str) -> orchester_core::account::AgentAccount {
  workspace
    .add_agent_account(NewAccount {
      cli: AccountCli::ClaudeCode,
      name: name(account_name),
      folder: NewFolder::Create,
    })
    .expect("added")
}

#[test]
fn accounts_removal_is_refused_while_a_step_continues_on_the_account_for_its_run() {
  let Setup { workspace, .. } = setup();
  let work = add(&workspace, "Work");
  let mut fixture = support::Fixture::new();
  workspace.open(fixture.repo.path()).expect("listed");
  let connection = connection_on(&mut fixture, "Claude · Work", &work);
  let task = fixture.queued_task("Sign in", support::at(10));
  let run = fixture.started_run(task, support::at(20));
  let step = run.snapshot().steps()[0].key().clone();
  fixture
    .db
    .set_step_connection(run.id(), &step, connection)
    .expect("continued on Work");
  let refused = workspace.remove_agent_account(work.id(), MoveTo::NoAccount, true);
  assert!(
    matches!(
      refused,
      Err(AccountsError::Removal(orchester_core::account::RemovalError::InUse))
    ),
    "{refused:?}"
  );
  assert!(
    work.folder().path().expect("a folder").is_dir(),
    "nothing went to the bin"
  );
}

#[test]
fn accounts_removal_is_refused_while_a_run_has_started_an_agent_on_the_account() {
  let Setup { workspace, .. } = setup();
  let (work, other) = (add(&workspace, "Work"), add(&workspace, "Other"));
  let mut fixture = support::Fixture::new();
  workspace.open(fixture.repo.path()).expect("listed");
  // The connection is on Other now; the run's agent started on Work before it was switched.
  connection_on(&mut fixture, "Claude", &other);
  let task = fixture.queued_task("Sign in", support::at(10));
  let run = fixture.started_run(task, support::at(20));
  let execution = run.current_attempt().expect("attempt").executions()[0].id();
  let invocation = orchester_core::store::ActivityStore::start_invocation(
    &mut fixture.db,
    run.id(),
    execution,
    &orchester_core::resource::ModelId::new("claude-sonnet-5-5").expect("a model"),
    support::at(21),
  )
  .expect("started");
  fixture
    .db
    .save_invocation_account(invocation.id, work.id())
    .expect("recorded");
  let refused = workspace.remove_agent_account(work.id(), MoveTo::NoAccount, true);
  assert!(
    matches!(
      refused,
      Err(AccountsError::Removal(orchester_core::account::RemovalError::InUse))
    ),
    "{refused:?}"
  );
  assert!(work.folder().path().expect("a folder").is_dir());
}

#[test]
fn accounts_removal_is_refused_while_a_listed_project_cannot_be_read() {
  let Setup { workspace, .. } = setup();
  let work = add(&workspace, "Work");
  let fixture = support::Fixture::new();
  workspace.open(fixture.repo.path()).expect("listed");
  let repo = fixture.into_repo();
  fs::remove_dir_all(repo.path().join(".orchester")).expect("its database is gone");
  let refused = workspace.remove_agent_account(work.id(), MoveTo::NoAccount, true);
  assert!(
    matches!(refused, Err(AccountsError::ProjectsUnreadable(_))),
    "{refused:?}"
  );
  assert!(work.folder().path().expect("a folder").is_dir());
}

/// Makes `link` lead to the folder `target`: a junction on Windows, a symbolic link elsewhere.
fn link(link: &Path, target: &Path) {
  #[cfg(windows)]
  {
    let made = std::process::Command::new("cmd")
      .args(["/C", "mklink", "/J"])
      .arg(link)
      .arg(target)
      .output()
      .expect("mklink runs");
    assert!(made.status.success(), "{made:?}");
  }
  #[cfg(not(windows))]
  std::os::unix::fs::symlink(target, link).expect("a link");
}

#[test]
fn accounts_a_brought_folder_is_judged_as_the_file_system_resolves_it() {
  let Setup { data, workspace } = setup();
  let (repo, _) = project(&workspace, "KON");
  let work = add(&workspace, "Work");
  let created = work.folder().path().expect("a folder").to_path_buf();
  let problem = |folder: &Path| workspace.brought_folder_problem(folder).expect("checked");
  assert_eq!(
    problem(&created.join("..").join("elsewhere")),
    Some(FolderProblem::NotAbsolute),
    "`..` is refused"
  );
  assert_eq!(
    problem(&created.join("inner")),
    Some(FolderProblem::Taken(work.id())),
    "inside another account's folder"
  );
  let target = repo.path().join(".claude-ci");
  fs::create_dir(&target).expect("a folder in the repository");
  let through = data.path().join("through-a-link");
  link(&through, &target);
  assert!(
    matches!(problem(&through), Some(FolderProblem::InsideRepository(_))),
    "a link into the repository"
  );
}

#[test]
fn accounts_a_created_folder_that_became_a_link_is_not_moved_to_the_bin() {
  let Setup { data, workspace } = setup();
  let work = add(&workspace, "Work");
  let created = work.folder().path().expect("a folder").to_path_buf();
  let elsewhere = data.path().join("the-persons-files");
  fs::create_dir(&elsewhere).expect("a folder of the person's");
  fs::write(elsewhere.join("notes.txt"), "keep").expect("a file");
  fs::remove_dir(&created).expect("the created folder goes");
  link(&created, &elsewhere);
  let refused = workspace.remove_agent_account(work.id(), MoveTo::NoAccount, true);
  assert!(
    matches!(
      refused,
      Err(AccountsError::Removal(
        orchester_core::account::RemovalError::FolderNotCreated
      ))
    ),
    "{refused:?}"
  );
  assert!(elsewhere.join("notes.txt").is_file(), "what the link leads to stays");
}

#[test]
fn accounts_a_new_accounts_folder_is_never_made_inside_a_repository() {
  let data = tempfile::tempdir().expect("temporary directory");
  let db = AppDb::open(&data.path().join("orchester.db")).expect("the app database");
  let probe = Workspace::new(db);
  let (repo, _) = project(&probe, "KON");
  let db = AppDb::open(&data.path().join("orchester.db")).expect("the app database");
  let workspace = Workspace::new(db).with_account_root(&repo.path().join("accounts"));
  let refused = workspace.add_agent_account(NewAccount {
    cli: AccountCli::Codex,
    name: name("Client"),
    folder: NewFolder::Create,
  });
  assert!(
    matches!(refused, Err(AccountsError::Folder(FolderProblem::InsideRepository(_)))),
    "{refused:?}"
  );
  assert!(!repo.path().join("accounts").exists(), "nothing is made");
}

#[test]
fn accounts_removal_moves_a_created_folder_to_the_trash_when_asked() {
  let Setup { data, workspace } = setup();
  let work = add(&workspace, "Work");
  let created = work.folder().path().expect("a folder").to_path_buf();
  workspace
    .remove_agent_account(work.id(), MoveTo::NoAccount, true)
    .expect("removed");
  assert!(!created.exists(), "the folder left its place");
  assert!(
    data.path().join("trash").join("claude-work").is_dir(),
    "it went to the test's own trash"
  );
}

#[test]
fn accounts_a_workspace_without_a_trash_says_so_and_keeps_the_folder() {
  let data = tempfile::tempdir().expect("temporary directory");
  let db = AppDb::open(&data.path().join("orchester.db")).expect("the app database");
  let workspace = Workspace::new(db).with_account_root(&data.path().join("accounts"));
  let work = add(&workspace, "Work");
  let refused = workspace.remove_agent_account(work.id(), MoveTo::NoAccount, true);
  assert!(matches!(refused, Err(AccountsError::NoTrash)), "{refused:?}");
  assert!(work.folder().path().expect("a folder").is_dir());
  assert!(
    workspace
      .agent_accounts()
      .expect("listed")
      .iter()
      .any(|account| account.id() == work.id()),
    "refused before anything is removed"
  );
}
