//! The link of a shared project to its server project (`docs/architecture/server.md`, "Desktop
//! data", "Migration"). Without a `sharing` row the project is local. While it is shared its tasks
//! are the cache of the server's, written only from what the server confirmed ([`super::shared_tasks`]):
//! the local writes of tasks, their organization and the project's tags refuse them
//! ([`StoreError::SharedTask`], [`StoreError::SharedProject`]), so nothing changes here that the
//! server does not have.

use orchester_core::{
  id::{ProjectId, ServerId, TaskId},
  project::ProjectKey,
  sharing::MemberRole,
  time::Timestamp,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{ProjectDb, joining::ReturnedTask};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// The server project a shared project's task list lives in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharingLink {
  pub server: ServerId,
  /// The project's id on the server.
  pub project: ProjectId,
  /// The server project's key, which the local project has too.
  pub key: ProjectKey,
  /// This person's role there as last read; `None` for an admin who is not a member.
  pub role: Option<MemberRole>,
  pub joined_at: Timestamp,
  /// Where the server's event stream continues for this computer.
  pub cursor: u64,
  /// When the cache last matched the server.
  pub last_synced_at: Timestamp,
  /// When the server answered that this person is no longer a member; the list is read-only since.
  pub removed_at: Option<Timestamp>,
  /// The project's address, its identity since API 3 (`<server>/<slug>`); `None` for a link made before, until the
  /// project is read from the server again.
  pub address: Option<String>,
}

impl ProjectDb {
  /// The project's server project; `None` while the project is local.
  pub fn sharing(&self) -> Result<Option<SharingLink>, StoreError> {
    link(&self.connection)
  }

  /// Records the project's address, read from the server for a link made before API 3.
  pub fn record_project_address(&mut self, address: &str) -> Result<(), StoreError> {
    let updated = self
      .connection
      .execute("UPDATE sharing SET project_address = ?1", [address])?;
    if updated == 0 {
      return Err(StoreError::NotShared);
    }
    Ok(())
  }

  /// Records this person's role in the server project, the time the cache matched it and whether
  /// they are still a member, as the server last answered.
  pub fn record_sharing_state(
    &mut self,
    role: Option<MemberRole>,
    synced_at: Option<Timestamp>,
    removed_at: Option<Timestamp>,
  ) -> Result<(), StoreError> {
    let updated = self.connection.execute(
      "UPDATE sharing SET role = ?1, last_synced_at = COALESCE(?2, last_synced_at), removed_at = ?3",
      params![
        role.map(role_text),
        synced_at.map(Timestamp::unix_millis),
        removed_at.map(Timestamp::unix_millis)
      ],
    )?;
    if updated == 0 {
      return Err(StoreError::NotShared);
    }
    Ok(())
  }

  /// Stops sharing on this computer (*Make local*): the link goes, and every cached task, with its
  /// id, number and organization as last synced, becomes a local task of the project; their
  /// pipelines are this computer's pipelines of their slugs. Claims and run statuses leave the
  /// cache, as do the members, and the next task number follows the highest. The project's tags, the server project's
  /// as last synced, become its local tags with their ids, as its modules, each task's module and their moves become
  /// its own. The tasks kept *Not shared* when it
  /// joined return to the list after the highest number; they are returned with their keys.
  pub fn make_local(&mut self) -> Result<Vec<ReturnedTask>, StoreError> {
    let transaction = self.connection.transaction()?;
    if link(&transaction)?.is_none() {
      return Err(StoreError::NotShared);
    }
    transaction.execute_batch(
      "UPDATE tasks SET pipeline_id = (SELECT resources.id FROM resources WHERE resources.kind = 'pipeline'          AND resources.slug = tasks.pipeline_slug) WHERE sharing = 'shared';
       UPDATE tasks SET sharing = NULL, pipeline_slug = NULL WHERE sharing = 'shared';",
    )?;
    let returned = super::joining::return_kept(&transaction, self.project.key())?;
    transaction.execute_batch(
      "DELETE FROM task_claims;
       DELETE FROM task_run_status;
       DELETE FROM shared_automatic_status;
       DELETE FROM shared_members;
       DELETE FROM sharing;
       UPDATE modules SET revision = NULL;
       UPDATE counters SET next = (SELECT COALESCE(MAX(number), 0) + 1 FROM tasks) WHERE name = 'task';",
    )?;
    transaction.commit()?;
    Ok(returned)
  }
}

pub(super) fn link(connection: &Connection) -> Result<Option<SharingLink>, StoreError> {
  connection
    .query_row(
      "SELECT server_id, project_id, project_key, role, joined_at, event_cursor, last_synced_at, removed_at, \
       project_address FROM sharing",
      [],
      LinkRow::read,
    )
    .optional()?
    .map(LinkRow::into_link)
    .transpose()
}

/// Stores `link` as the project's only link, in the transaction that fills the cache.
pub(super) fn insert_link(connection: &Connection, link: &SharingLink) -> Result<(), StoreError> {
  connection.execute(
    "INSERT INTO sharing (only_row, server_id, project_id, project_key, role, joined_at, event_cursor, last_synced_at, \
     removed_at, project_address) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    params![
      link.server.to_string(),
      link.project.to_string(),
      link.key.as_str(),
      link.role.map(role_text),
      link.joined_at.unix_millis(),
      crate::rows::tokens_column("event cursor", link.cursor)?,
      link.last_synced_at.unix_millis(),
      link.removed_at.map(Timestamp::unix_millis),
      link.address,
    ],
  )?;
  Ok(())
}

/// Fails while the project is shared: its new tasks and its tags are made on the server.
pub(super) fn ensure_local_project(connection: &Connection) -> Result<(), StoreError> {
  let shared: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM sharing)", [], |row| row.get(0))?;
  if shared {
    return Err(StoreError::SharedProject);
  }
  Ok(())
}

/// Fails when `task` is shared, as it changes on the server only, or kept *Not shared*, as it is
/// read-only.
pub(super) fn ensure_local_task(connection: &Connection, task: TaskId) -> Result<(), StoreError> {
  let sharing: Option<String> = connection
    .query_row("SELECT sharing FROM tasks WHERE id = ?1", [task.to_string()], |row| {
      row.get(0)
    })
    .optional()?
    .flatten();
  match sharing.as_deref() {
    Some("not_shared") => Err(StoreError::NotSharedTask(task)),
    Some(_) => Err(StoreError::SharedTask(task)),
    None => Ok(()),
  }
}

pub(super) const fn role_text(role: MemberRole) -> &'static str {
  match role {
    MemberRole::Owner => "owner",
    MemberRole::Member => "member",
  }
}

pub(super) fn parse_role(value: &str) -> Result<MemberRole, StoreError> {
  match value {
    "owner" => Ok(MemberRole::Owner),
    "member" => Ok(MemberRole::Member),
    other => Err(corrupt("member role", other)),
  }
}

struct LinkRow {
  server: String,
  project: String,
  key: String,
  role: Option<String>,
  joined_at: i64,
  cursor: i64,
  last_synced_at: i64,
  removed_at: Option<i64>,
  address: Option<String>,
}

impl LinkRow {
  fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
    Ok(Self {
      server: row.get(0)?,
      project: row.get(1)?,
      key: row.get(2)?,
      role: row.get(3)?,
      joined_at: row.get(4)?,
      cursor: row.get(5)?,
      last_synced_at: row.get(6)?,
      removed_at: row.get(7)?,
      address: row.get(8)?,
    })
  }

  fn into_link(self) -> Result<SharingLink, StoreError> {
    Ok(SharingLink {
      server: parse("server id", &self.server)?,
      project: parse("server project id", &self.project)?,
      key: ProjectKey::new(&self.key).map_err(|error| corrupt("server project key", error))?,
      role: self.role.as_deref().map(parse_role).transpose()?,
      joined_at: Timestamp::from_unix_millis(self.joined_at),
      cursor: u64::try_from(self.cursor).map_err(|error| corrupt("event cursor", error))?,
      last_synced_at: Timestamp::from_unix_millis(self.last_synced_at),
      removed_at: self.removed_at.map(Timestamp::from_unix_millis),
      address: self.address,
    })
  }
}
