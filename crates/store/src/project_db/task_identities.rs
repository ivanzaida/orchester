//! Task identities (`orchester_core::task::identity`, `schema::task_identities`): what a task's records refer to, so
//! they stay whole without its definition. A local task's identity is written with its definition. A team task's names
//! the server's task: written for a cached shared task while the cache lasts, and for a team task this computer keeps
//! no definition of ([`ProjectDb::record_team_task`]).

use orchester_core::{
  id::TaskId,
  sharing::TaskRevision,
  task::identity::{TaskIdentity, TaskOwner, TeamTaskRef},
};
use rusqlite::{Connection, OptionalExtension, params};

use super::{ProjectDb, sharing, tasks::revision_value};
use crate::{
  StoreError,
  rows::{parse, tokens_column},
};

impl ProjectDb {
  /// The identity of task `id`; `None` when no record of this project names it.
  pub fn task_identity(&self, id: TaskId) -> Result<Option<TaskIdentity>, StoreError> {
    load(&self.connection, id)
  }

  /// Records the identity of the team task `team`, at `revision`, without its definition, and returns the id this
  /// project's records name it by: the identity recorded before for it, or a new one with the server's task id. The
  /// project must be linked to the team task's server project; a local task with the server's task id is
  /// [`StoreError::TaskIdTaken`].
  pub fn record_team_task(&mut self, team: TeamTaskRef, revision: TaskRevision) -> Result<TaskId, StoreError> {
    let transaction = self.connection.transaction()?;
    let link = sharing::link(&transaction)?.ok_or(StoreError::NotShared)?;
    if link.server != team.server || link.project != team.project {
      return Err(StoreError::OtherServerProject);
    }
    let known: Option<String> = transaction
      .query_row(
        "SELECT id FROM task_identities WHERE server_id = ?1 AND team_project_id = ?2 AND team_task_id = ?3",
        params![team.server.to_string(), team.project.to_string(), team.task.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    if let Some(id) = known {
      return parse("task id", &id);
    }
    if identity_exists(&transaction, team.task)? {
      return Err(StoreError::TaskIdTaken(team.task));
    }
    insert_team(&transaction, team.task, team, revision)?;
    transaction.commit()?;
    Ok(team.task)
  }
}

/// Records the identity of the new local task `task`, before its definition.
pub(super) fn insert_local(connection: &Connection, task: TaskId) -> Result<(), StoreError> {
  connection.execute(
    "INSERT INTO task_identities (id, owner) VALUES (?1, 'local')",
    [task.to_string()],
  )?;
  Ok(())
}

/// Records the identity of the shared task `task` the server confirmed at `revision`, before its cached definition.
/// A local task the project shared becomes the server project's; a known team identity stays as it was recorded.
pub(super) fn record_cached(connection: &Connection, task: TaskId, revision: TaskRevision) -> Result<(), StoreError> {
  let link = sharing::link(connection)?.ok_or(StoreError::NotShared)?;
  connection.execute(
    "INSERT INTO task_identities (id, owner, server_id, team_project_id, team_task_id, recorded_revision) \
     VALUES (?1, 'team', ?2, ?3, ?1, ?4) \
     ON CONFLICT (id) DO UPDATE SET owner = 'team', server_id = excluded.server_id, \
     team_project_id = excluded.team_project_id, team_task_id = excluded.team_task_id, \
     recorded_revision = excluded.recorded_revision WHERE task_identities.owner = 'local'",
    params![
      task.to_string(),
      link.server.to_string(),
      link.project.to_string(),
      tokens_column("task revision", revision.get())?
    ],
  )?;
  Ok(())
}

fn insert_team(
  connection: &Connection,
  id: TaskId,
  team: TeamTaskRef,
  revision: TaskRevision,
) -> Result<(), StoreError> {
  connection.execute(
    "INSERT INTO task_identities (id, owner, server_id, team_project_id, team_task_id, recorded_revision) \
     VALUES (?1, 'team', ?2, ?3, ?4, ?5)",
    params![
      id.to_string(),
      team.server.to_string(),
      team.project.to_string(),
      team.task.to_string(),
      tokens_column("task revision", revision.get())?
    ],
  )?;
  Ok(())
}

/// Fails with [`StoreError::TaskNotFound`] when no identity is `task`.
pub(super) fn ensure_known(connection: &Connection, task: TaskId) -> Result<(), StoreError> {
  if !identity_exists(connection, task)? {
    return Err(StoreError::TaskNotFound(task));
  }
  Ok(())
}

fn identity_exists(connection: &Connection, task: TaskId) -> Result<bool, StoreError> {
  Ok(connection.query_row(
    "SELECT EXISTS (SELECT 1 FROM task_identities WHERE id = ?1)",
    [task.to_string()],
    |row| row.get(0),
  )?)
}

/// Whether this computer keeps a definition of `task`: a team task's may be only on its server.
pub(super) fn has_definition(connection: &Connection, task: TaskId) -> Result<bool, StoreError> {
  Ok(connection.query_row(
    "SELECT EXISTS (SELECT 1 FROM tasks WHERE id = ?1)",
    [task.to_string()],
    |row| row.get(0),
  )?)
}

type IdentityRow = (String, Option<String>, Option<String>, Option<String>, Option<i64>);

pub(super) fn load(connection: &Connection, id: TaskId) -> Result<Option<TaskIdentity>, StoreError> {
  let row: Option<IdentityRow> = connection
    .query_row(
      "SELECT owner, server_id, team_project_id, team_task_id, recorded_revision FROM task_identities WHERE id = ?1",
      [id.to_string()],
      |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    )
    .optional()?;
  let Some((owner, server, project, task, revision)) = row else {
    return Ok(None);
  };
  let owner = match (owner.as_str(), server, project, task, revision) {
    ("local", None, None, None, None) => TaskOwner::Local,
    ("team", Some(server), Some(project), Some(task), Some(revision)) => TaskOwner::Team {
      task: TeamTaskRef {
        server: parse("team server id", &server)?,
        project: parse("team project id", &project)?,
        task: parse("team task id", &task)?,
      },
      recorded_revision: revision_value(revision)?,
    },
    (other, ..) => return Err(crate::rows::corrupt("task owner", other)),
  };
  Ok(Some(TaskIdentity { id, owner }))
}
