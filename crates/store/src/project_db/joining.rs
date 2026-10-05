//! Joining a shared repository (`docs/architecture/server.md`, "Two people share one repository";
//! "Migration"): the local tasks a person ticked were uploaded with new numbers and become the
//! server's like every other cached task; the ones they unticked stay here as *Not shared*, out of
//! the task list, read-only, with the key they had and their tag names. The keys every local task
//! had are kept for the mapping until *Make local*, which returns the Not shared tasks to the list
//! with numbers after the highest.

use std::{collections::HashMap, num::NonZeroU32};

use orchester_core::{
  id::{TagId, TaskId},
  name::Name,
  project::{ProjectKey, TaskKey},
  task::{Task, organization::Tag},
  time::Timestamp,
};
use rusqlite::{Connection, Transaction, params};

use super::{
  ProjectDb, organization,
  shared_tasks::{self, CachedTask},
  sharing::{self, SharingLink},
  tasks,
};
use crate::{
  StoreError,
  rows::{corrupt, decode_json, encode_json, parse},
};

/// What a local task became when the project joined, as Sharing shows it (10.22).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinedKey {
  pub task: TaskId,
  /// The key it had here before joining.
  pub former: String,
  /// Its key now: the server project's, or the kept one while it is *Not shared*.
  pub key: TaskKey,
  pub not_shared: bool,
  pub title: Name,
  /// The run it was uploaded claimed for, when that run had not ended.
  pub claimed_run: Option<NonZeroU32>,
}

/// A *Not shared* task back in the list after *Make local*, with its key before and after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReturnedTask {
  pub task: TaskId,
  pub before: TaskKey,
  pub after: TaskKey,
  pub title: Name,
}

impl ProjectDb {
  /// Joins the server project of `link` (*Join*): every local task not in `tasks` is kept *Not
  /// shared* with its key and tag names, the server project's `tags` and `tasks` (the ticked ones
  /// uploaded among them) are cached, the keys of the local tasks are kept for the mapping, with
  /// the runs in `claimed` their tasks were claimed for, and the project takes the server project's
  /// key. One transaction.
  pub fn join_sharing(
    &mut self,
    link: &SharingLink,
    (tags, tasks): (&[Tag], &[CachedTask]),
    claimed: &HashMap<TaskId, NonZeroU32>,
  ) -> Result<(), StoreError> {
    // Immediate: the runner may write between the read and the first write of a deferred one.
    let transaction = self
      .connection
      .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    if sharing::link(&transaction)?.is_some() {
      return Err(StoreError::AlreadyShared);
    }
    let local = shared_tasks::project_key(&transaction)?;
    for task in tasks::load_tasks(&transaction, &local, "WHERE sharing IS NULL ORDER BY number", [])? {
      keep(&transaction, &task, claimed.get(&task.id()).copied())?;
    }
    sharing::insert_link(&transaction, link)?;
    shared_tasks::replace_tags(&transaction, tags)?;
    for task in tasks {
      shared_tasks::write_cached(&transaction, task, None)?;
    }
    transaction.execute("UPDATE project SET key = ?1", [link.key.as_str()])?;
    transaction.commit()?;
    self.project = super::load_project(&self.connection)?;
    Ok(())
  }

  /// The tasks kept *Not shared* when the project joined, by their kept key.
  pub fn not_shared_tasks(&self) -> Result<Vec<Task>, StoreError> {
    tasks::load_tasks(
      &self.connection,
      self.project.key(),
      "WHERE sharing = 'not_shared' ORDER BY number",
      [],
    )
  }

  /// The key each task of the list had before it was uploaded (*was KON-12*), for its page and the
  /// list's search.
  pub fn former_keys(&self) -> Result<HashMap<TaskId, String>, StoreError> {
    let mut statement = self
      .connection
      .prepare("SELECT id, former_key FROM tasks WHERE former_key IS NOT NULL AND sharing IS NOT 'not_shared'")?;
    let rows = statement
      .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(id, key)| Ok((parse("task id", &id)?, key)))
      .collect()
  }

  /// The keys the local tasks had when the project joined, the uploaded ones by their new number
  /// first, then the *Not shared* ones; empty for a project that did not join.
  pub fn joined_keys(&self) -> Result<Vec<JoinedKey>, StoreError> {
    let mut statement = self.connection.prepare(
      "SELECT joined_keys.task_id, joined_keys.former_key, joined_keys.claimed_run FROM joined_keys \
       JOIN tasks ON tasks.id = joined_keys.task_id \
       ORDER BY tasks.sharing IS 'not_shared', tasks.number",
    )?;
    let rows = statement
      .query_map([], |row| {
        Ok((
          row.get::<_, String>(0)?,
          row.get::<_, String>(1)?,
          row.get::<_, Option<i64>>(2)?,
        ))
      })?
      .collect::<Result<Vec<_>, _>>()?;
    rows
      .into_iter()
      .map(|(id, former, claimed)| {
        let task = tasks::load_tasks(&self.connection, self.project.key(), "WHERE id = ?1", [&id])?
          .pop()
          .ok_or_else(|| corrupt("joined task", &id))?;
        let not_shared = shared_tasks::is_not_shared(&self.connection, task.id())?;
        Ok(JoinedKey {
          task: task.id(),
          former,
          key: task.key().clone(),
          not_shared,
          title: task.details().title.clone(),
          claimed_run: claimed.map(tasks::task_number).transpose()?,
        })
      })
      .collect()
  }
}

/// Keeps `task` here *Not shared* (it may still be uploaded in the same transaction, which makes
/// it the server's), with its key and tag names, and records its key for the mapping.
fn keep(transaction: &Transaction<'_>, task: &Task, claimed: Option<NonZeroU32>) -> Result<(), StoreError> {
  let organization = organization::load(transaction, task.id())?;
  let names = tag_names(transaction, organization.tags())?;
  transaction.execute(
    "UPDATE tasks SET sharing = 'not_shared', kept_key = ?2, kept_tag_names = ?3 WHERE id = ?1",
    params![
      task.id().to_string(),
      task.key().project().as_str(),
      encode_json("kept tag names", 1, &names)?
    ],
  )?;
  // The project's tags become the server's: the kept names stand for the task's tags meanwhile.
  organization::store_cached(
    transaction,
    task.id(),
    &organization.with_tags(Vec::new()),
    Timestamp::now(),
  )?;
  transaction.execute(
    "INSERT INTO joined_keys (task_id, former_key, claimed_run) VALUES (?1, ?2, ?3)",
    params![
      task.id().to_string(),
      task.key().to_string(),
      claimed.map(NonZeroU32::get)
    ],
  )?;
  Ok(())
}

fn tag_names(connection: &Connection, tags: &[TagId]) -> Result<Vec<String>, StoreError> {
  tags
    .iter()
    .map(|id| {
      Ok(
        connection.query_row("SELECT name FROM project_tags WHERE id = ?1", [id.to_string()], |row| {
          row.get(0)
        })?,
      )
    })
    .collect()
}

/// Returns the *Not shared* tasks to the list (*Make local*) with numbers after the highest, in the
/// order of their kept numbers. Each keeps its key before as its former key and gets its kept tags
/// back by name, from the project's tags or as new ones.
pub(super) fn return_kept(
  transaction: &Transaction<'_>,
  project: &ProjectKey,
) -> Result<Vec<ReturnedTask>, StoreError> {
  let kept = tasks::load_tasks(transaction, project, "WHERE sharing = 'not_shared' ORDER BY number", [])?;
  let highest: i64 = transaction.query_row(
    "SELECT COALESCE(MAX(number), 0) FROM tasks WHERE sharing IS NOT 'not_shared'",
    [],
    |row| row.get(0),
  )?;
  let mut returned = Vec::with_capacity(kept.len());
  for (number, task) in (highest + 1..).zip(kept) {
    let names: String = transaction.query_row(
      "SELECT kept_tag_names FROM tasks WHERE id = ?1",
      [task.id().to_string()],
      |row| row.get(0),
    )?;
    let names: Vec<String> = decode_json("kept tag names", 1, &names)?;
    let tags = names
      .iter()
      .map(|name| local_tag(transaction, name))
      .collect::<Result<Vec<_>, _>>()?;
    let organization = organization::load(transaction, task.id())?.with_tags(tags);
    organization::store_cached(transaction, task.id(), &organization, Timestamp::now())?;
    transaction.execute(
      "UPDATE tasks SET number = ?2, former_key = ?3, sharing = NULL, kept_key = NULL, kept_tag_names = NULL \
       WHERE id = ?1",
      params![task.id().to_string(), number, task.key().to_string()],
    )?;
    returned.push(ReturnedTask {
      task: task.id(),
      after: TaskKey::new(project.clone(), tasks::task_number(number)?),
      before: task.key().clone(),
      title: task.details().title.clone(),
    });
  }
  transaction.execute("DELETE FROM joined_keys", [])?;
  Ok(returned)
}

/// The project's tag named `name` without regard to case, made when it has none.
fn local_tag(connection: &Connection, name: &str) -> Result<TagId, StoreError> {
  let found: Option<String> = rusqlite::OptionalExtension::optional(connection.query_row(
    "SELECT id FROM project_tags WHERE normalized = ?1",
    [name.to_lowercase()],
    |row| row.get(0),
  ))?;
  if let Some(id) = found {
    return parse("tag id", &id);
  }
  let name = Name::new(name).map_err(|error| corrupt("kept tag name", error))?;
  let id = TagId::generate();
  connection.execute(
    "INSERT INTO project_tags (id, name, normalized) VALUES (?1, ?2, ?3)",
    params![id.to_string(), name.as_str(), name.as_str().to_lowercase()],
  )?;
  Ok(id)
}
