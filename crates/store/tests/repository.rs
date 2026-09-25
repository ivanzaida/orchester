//! The project's GitHub repository link.

use std::fs;

use orchester_core::{
  connection::{AgentConnection, AgentProvider, GitHubConnection, GitHubHost, GitHubLogin},
  id::{ConnectionId, ProjectId},
  name::Name,
  project::{Project, ProjectKey},
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryLink, RepositoryView},
  run::BranchName,
  store::{ConnectionStore, RepositoryStore},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};
use rusqlite::Connection as Sqlite;
use tempfile::TempDir;

fn project_repo() -> (TempDir, ProjectDb) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("valid key"),
    Name::new("Kontur").expect("valid name"),
  );
  let db = ProjectDb::create(repo.path(), project).expect("create project");
  (repo, db)
}

fn github(repo: &TempDir, db: &mut ProjectDb) -> GitHubConnection {
  let connection = GitHubConnection::new(
    ConnectionId::generate(),
    Name::new("GitHub").expect("valid name"),
    GitHubHost::github_com(),
    repo.path().join("gh.exe"),
    Timestamp::from_unix_millis(1_000),
  )
  .expect("absolute executable");
  db.create_connection(&connection).expect("create connection");
  connection
}

fn link(connection: ConnectionId, name: &str, permission: RepoPermission) -> RepositoryLink {
  let view = RepositoryView {
    repo: GitHubRepo::new(GitHubHost::github_com(), "ivanzaida", name).expect("valid repository"),
    default_branch: BranchName::new("master").expect("valid branch"),
    permission,
  };
  RepositoryLink::new(
    connection,
    RemoteName::origin(),
    view,
    GitHubLogin::new("ivanzaida").expect("valid login"),
    Timestamp::from_unix_millis(2_000),
  )
}

#[test]
fn repository_link_round_trip_and_replacement() {
  let (repo, mut db) = project_repo();
  assert_eq!(db.repository_link().expect("read"), None);
  let connection = github(&repo, &mut db);
  let first = link(connection.id(), "orchester", RepoPermission::Admin);
  db.save_repository_link(&first).expect("save");
  assert_eq!(db.repository_link().expect("read"), Some(first));

  let second = link(connection.id(), "lurq", RepoPermission::Read);
  db.save_repository_link(&second).expect("replace");
  let reopened = ProjectDb::open(repo.path()).expect("reopen");
  assert_eq!(reopened.repository_link().expect("read"), Some(second));
  let raw = Sqlite::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  let rows: i64 = raw
    .query_row("SELECT count(*) FROM repository_link", [], |row| row.get(0))
    .expect("count");
  assert_eq!(rows, 1);

  db.remove_repository_link().expect("remove");
  assert_eq!(db.repository_link().expect("read"), None);
  db.remove_repository_link().expect("removing nothing is fine");
}

#[test]
fn repository_link_needs_a_github_connection() {
  let (repo, mut db) = project_repo();
  let unknown = ConnectionId::generate();
  assert!(matches!(
    db.save_repository_link(&link(unknown, "orchester", RepoPermission::Write)),
    Err(StoreError::ConnectionNotFound(id)) if id == unknown
  ));
  let codex = AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Codex").expect("valid name"),
    AgentProvider::CodexCli,
    repo.path().join("codex.exe"),
    Timestamp::from_unix_millis(1_000),
  )
  .expect("absolute executable");
  db.create_connection(&codex).expect("create connection");
  assert!(matches!(
    db.save_repository_link(&link(codex.id(), "orchester", RepoPermission::Write)),
    Err(StoreError::NotAGitHubConnection(id)) if id == codex.id()
  ));
  assert_eq!(db.repository_link().expect("read"), None);
}

#[test]
fn damaged_repository_link_is_reported() {
  let (repo, mut db) = project_repo();
  let connection = github(&repo, &mut db);
  db.save_repository_link(&link(connection.id(), "orchester", RepoPermission::Admin))
    .expect("save");
  let raw = Sqlite::open(repo.path().join(".orchester/orchester.db")).expect("open raw database");
  raw
    .execute("UPDATE repository_link SET expected_login = 'not a login'", [])
    .expect("damage the row");
  assert!(matches!(
    db.repository_link(),
    Err(StoreError::Corrupt {
      what: "repository login",
      ..
    })
  ));
}
