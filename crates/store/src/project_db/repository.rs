//! The `repository_link` table: the project's GitHub repository, at most one row.

use orchester_core::{
  connection::{GitHubHost, GitHubLogin},
  id::ConnectionId,
  repository::{GitHubRepo, RemoteName, RepoPermission, RepositoryLink, RepositoryView},
  run::BranchName,
  store::RepositoryStore,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, Row, params};

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// The only row id: the table holds one link at most.
const LINK_ROW: i64 = 1;
const GITHUB_KIND: &str = "github";

impl RepositoryStore for ProjectDb {
  type Error = StoreError;

  fn repository_link(&self) -> Result<Option<RepositoryLink>, StoreError> {
    let row = self
      .connection
      .query_row(
        "SELECT connection_id, remote, host, owner, name, default_branch, permission, expected_login, checked_at \
         FROM repository_link WHERE id = ?1",
        [LINK_ROW],
        StoredLink::read,
      )
      .optional()?;
    row.map(StoredLink::into_link).transpose()
  }

  fn save_repository_link(&mut self, link: &RepositoryLink) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    let kind: Option<String> = transaction
      .query_row(
        "SELECT kind FROM connections WHERE id = ?1",
        [link.connection().to_string()],
        |row| row.get(0),
      )
      .optional()?;
    match kind.as_deref() {
      None => return Err(StoreError::ConnectionNotFound(link.connection())),
      Some(GITHUB_KIND) => {}
      Some(_) => return Err(StoreError::NotAGitHubConnection(link.connection())),
    }
    let repo = link.repo();
    transaction.execute(
      "INSERT OR REPLACE INTO repository_link \
       (id, connection_id, remote, host, owner, name, default_branch, permission, expected_login, checked_at) \
       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
      params![
        LINK_ROW,
        link.connection().to_string(),
        link.remote().as_str(),
        repo.host().as_str(),
        repo.owner(),
        repo.name(),
        link.default_branch().as_str(),
        link.permission().as_str(),
        link.expected_login().as_str(),
        link.checked_at().unix_millis()
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  fn remove_repository_link(&mut self) -> Result<(), StoreError> {
    self
      .connection
      .execute("DELETE FROM repository_link WHERE id = ?1", [LINK_ROW])?;
    Ok(())
  }
}

struct StoredLink {
  connection: String,
  remote: String,
  host: String,
  owner: String,
  name: String,
  default_branch: String,
  permission: String,
  expected_login: String,
  checked_at: i64,
}

impl StoredLink {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      connection: row.get(0)?,
      remote: row.get(1)?,
      host: row.get(2)?,
      owner: row.get(3)?,
      name: row.get(4)?,
      default_branch: row.get(5)?,
      permission: row.get(6)?,
      expected_login: row.get(7)?,
      checked_at: row.get(8)?,
    })
  }

  fn into_link(self) -> Result<RepositoryLink, StoreError> {
    let host = GitHubHost::new(&self.host).map_err(|error| corrupt("repository host", error))?;
    let view = RepositoryView {
      repo: GitHubRepo::new(host, &self.owner, &self.name).map_err(|error| corrupt("repository", error))?,
      default_branch: BranchName::new(&self.default_branch)
        .map_err(|error| corrupt("repository default branch", error))?,
      permission: RepoPermission::parse(&self.permission)
        .ok_or_else(|| corrupt("repository permission", &self.permission))?,
    };
    Ok(RepositoryLink::new(
      parse::<ConnectionId>("repository connection", &self.connection)?,
      RemoteName::new(&self.remote).map_err(|error| corrupt("repository remote", error))?,
      view,
      GitHubLogin::new(&self.expected_login).map_err(|error| corrupt("repository login", error))?,
      Timestamp::from_unix_millis(self.checked_at),
    ))
  }
}
