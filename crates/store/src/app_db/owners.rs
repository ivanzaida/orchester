//! Whose each known project is (the person's decision, 2026-10-16: projects per account): the Orchester Cloud account
//! that was signed in when it was added, by the cloud's account id. A project added while no account was signed in has
//! no owner, and the next account to sign in takes it ([`AppDb::claim_unowned_projects`]). Which account's list is
//! shown is the workspace's decision; these are the rows it reads and writes.

use std::path::Path;

use orchester_core::{
  id::{CloudAccountId, ProjectId},
  project::Project,
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::{AppDb, RegisteredProject};
use crate::{StoreError, rows::path_text};

/// The projects known before this change belong to the account signed in then, if one was; the others wait for the
/// first account to sign in.
pub(super) const MIGRATION: &str = r"
ALTER TABLE projects ADD COLUMN owner TEXT;
UPDATE projects SET owner = (SELECT account_id FROM cloud_account WHERE id = 1);
";

const COLUMNS: &str = "SELECT id, name, repo_root, last_opened_at FROM projects";

impl AppDb {
  /// The projects of `owner`, most recently opened first; `None` lists those no account took yet.
  pub fn projects_of(&self, owner: Option<&CloudAccountId>) -> Result<Vec<RegisteredProject>, StoreError> {
    let mut statement = self
      .connection
      .prepare(&format!("{COLUMNS} WHERE owner IS ?1 ORDER BY last_opened_at DESC"))?;
    let rows = statement
      .query_map([owner.map(ToString::to_string)], super::project_row)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(super::registered).collect()
  }

  /// Every known project, whoever's it is, most recently opened first: for the work that follows every project on
  /// this computer (an agent account's folders, a shared project's sync), never to show.
  pub fn every_project(&self) -> Result<Vec<RegisteredProject>, StoreError> {
    let mut statement = self
      .connection
      .prepare(&format!("{COLUMNS} ORDER BY last_opened_at DESC"))?;
    let rows = statement
      .query_map([], super::project_row)?
      .collect::<Result<Vec<_>, _>>()?;
    rows.into_iter().map(super::registered).collect()
  }

  /// Adds the project for `owner` or, if it is already known, updates its name, location and last use; a project
  /// no account took yet becomes `owner`'s. A project of another account is refused
  /// ([`StoreError::ProjectOfAnotherAccount`]) and stays as it was.
  pub fn register_project(
    &mut self,
    project: &Project,
    repo_root: &Path,
    now: Timestamp,
    owner: Option<&CloudAccountId>,
  ) -> Result<(), StoreError> {
    let owner = owner.map(ToString::to_string);
    let transaction = self.connection.transaction()?;
    let known: Option<Option<String>> = transaction
      .query_row(
        "SELECT owner FROM projects WHERE id = ?1",
        [project.id().to_string()],
        |row| row.get(0),
      )
      .optional()?;
    if let Some(Some(known)) = known
      && Some(&known) != owner.as_ref()
    {
      return Err(StoreError::ProjectOfAnotherAccount(project.id()));
    }
    transaction.execute(
      "INSERT INTO projects (id, name, repo_root, added_at, last_opened_at, owner) VALUES (?1, ?2, ?3, ?4, ?4, ?5) \
       ON CONFLICT (id) DO UPDATE SET name = excluded.name, repo_root = excluded.repo_root, \
       last_opened_at = excluded.last_opened_at, owner = COALESCE(projects.owner, excluded.owner)",
      params![
        project.id().to_string(),
        project.name().as_str(),
        path_text("repository path", repo_root)?,
        now.unix_millis(),
        owner,
      ],
    )?;
    transaction.commit()?;
    Ok(())
  }

  /// Forgets `owner`'s project `project`: it is no longer listed. Only this database changes; the project's folder
  /// and its `.orchester` data stay as they are, so opening the folder again brings the project back. Forgetting a
  /// project that is not known, or not `owner`'s, changes nothing.
  pub fn forget_project(&mut self, project: ProjectId, owner: Option<&CloudAccountId>) -> Result<(), StoreError> {
    self.connection.execute(
      "DELETE FROM projects WHERE id = ?1 AND owner IS ?2",
      params![project.to_string(), owner.map(ToString::to_string)],
    )?;
    Ok(())
  }

  /// Gives `owner` every project no account took yet: those added while none was signed in, and those known before
  /// projects had owners.
  pub fn claim_unowned_projects(&mut self, owner: &CloudAccountId) -> Result<(), StoreError> {
    self.connection.execute(
      "UPDATE projects SET owner = ?1 WHERE owner IS NULL",
      [owner.to_string()],
    )?;
    Ok(())
  }
}
