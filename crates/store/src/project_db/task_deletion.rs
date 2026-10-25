//! Deleting a task (ORC-91): a person's decision to drop the task with everything recorded about it. Its runs go
//! with their history (attempts, steps, events, transcripts, plans, questions, approvals, usage), its conversation
//! with the copies of its attachments, its memory records, its organization and its history, and whatever else the
//! schema keeps of it or of its runs: every row that refers to them through a foreign key, followed to the rows that
//! refer to those, so no reference is left dangling. That includes the project's memory records its runs' agents wrote
//! and the project approval rules approved in its runs, which [`TaskDeletionFacts`] counts for the confirmation. The
//! tasks it blocked lose their link to it. Rows the schema otherwise never deletes (run events, checkpoints, plans,
//! messages…) lose their guard for this one transaction only.
//!
//! A shared project's tasks are the server's, which keeps no deletion ([`StoreError::SharedProject`]); a task whose
//! run has not ended is not deleted ([`StoreError::TaskRunActive`]). What its runs left in git is not this module's.

use std::collections::{BTreeMap, BTreeSet};

use orchester_core::{id::TaskId, task::organization::ChangeActor, time::Timestamp};
use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{ProjectDb, attachments, organization, sharing};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// What deleting a task takes with it besides the task itself, for its confirmation to name.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TaskDeletionFacts {
  pub runs: usize,
  /// The task's own memory records.
  pub memories: usize,
  /// The project's memory records its runs' agents wrote.
  pub project_memories: usize,
  /// The project approval rules approved in its runs; later runs ask again.
  pub project_rules: usize,
  /// The tasks it blocks, which lose their link to it.
  pub blocking: usize,
}

/// A foreign key: `columns` of `table` refer to `parent_columns` of `parent`.
struct Reference {
  table: String,
  columns: Vec<String>,
  parent: String,
  parent_columns: Vec<String>,
}

impl ProjectDb {
  /// What deleting `task` takes with it.
  pub fn task_deletion_facts(&self, task: TaskId) -> Result<TaskDeletionFacts, StoreError> {
    organization::load(&self.connection, task)?;
    let id = task.to_string();
    let count = |sql: &str| -> Result<usize, StoreError> {
      let found: i64 = self.connection.query_row(sql, [&id], |row| row.get(0))?;
      usize::try_from(found).map_err(|error| corrupt("row count", error))
    };
    Ok(TaskDeletionFacts {
      runs: count("SELECT COUNT(*) FROM runs WHERE task_id = ?1")?,
      memories: count("SELECT COUNT(*) FROM memories WHERE task_id = ?1")?,
      project_memories: count(
        "SELECT COUNT(*) FROM memories WHERE task_id IS NULL AND run_id IN (SELECT id FROM runs WHERE task_id = ?1)",
      )?,
      project_rules: count(
        "SELECT COUNT(*) FROM project_approval_rules WHERE made_in IN (SELECT id FROM runs WHERE task_id = ?1)",
      )?,
      blocking: blocked_by(&self.connection, task)?.len(),
    })
  }

  /// Deletes `task` with everything recorded about it, at `at`, in one transaction, then the copies of its
  /// attachments. Refused in a shared project and while a run of the task has not ended; nothing is deleted then.
  pub fn delete_task(&mut self, task: TaskId, at: Timestamp) -> Result<(), StoreError> {
    let copies: Vec<_> = self
      .message_attachments(task)?
      .into_values()
      .flatten()
      .map(|attachment| self.attachment_path(&attachment))
      .collect();
    let transaction = self.connection.transaction()?;
    if sharing::link(&transaction)?.is_some() {
      return Err(StoreError::SharedProject);
    }
    organization::load(&transaction, task)?;
    if let Some(run) = organization::active_run(&transaction, task)? {
      return Err(StoreError::TaskRunActive(run));
    }
    for blocked in blocked_by(&transaction, task)? {
      let value = organization::load(&transaction, blocked)?.without_blocking(task);
      organization::write(&transaction, blocked, &value, ChangeActor::Person, at)?;
    }
    // The rows refer to each other until the last of them is gone.
    transaction.pragma_update(None, "defer_foreign_keys", true)?;
    let doomed = doomed_rows(&transaction, task)?;
    delete_unguarded(&transaction, &doomed)?;
    transaction.commit()?;
    attachments::remove_copies(&copies);
    Ok(())
  }
}

/// The tasks whose organization names `task` as blocking them.
fn blocked_by(connection: &Connection, task: TaskId) -> Result<Vec<TaskId>, StoreError> {
  let mut statement = connection.prepare("SELECT task_id FROM task_organization WHERE task_id <> ?1")?;
  let ids = statement
    .query_map([task.to_string()], |row| row.get::<_, String>(0))?
    .collect::<Result<Vec<_>, _>>()?;
  let mut blocked = Vec::new();
  for id in ids {
    let other = parse("task id", &id)?;
    let value = organization::load(connection, other)?;
    if value.dependencies().iter().any(|link| link.task == task) {
      blocked.push(other);
    }
  }
  Ok(blocked)
}

/// The rows to delete, by table and rowid: the task's, and every row that refers to one of them, to the last.
fn doomed_rows(connection: &Connection, task: TaskId) -> Result<BTreeMap<String, BTreeSet<i64>>, StoreError> {
  let references = references(connection)?;
  let rowid: Option<i64> = connection
    .query_row("SELECT rowid FROM tasks WHERE id = ?1", [task.to_string()], |row| {
      row.get(0)
    })
    .optional()?;
  let rowid = rowid.ok_or(StoreError::TaskNotFound(task))?;
  let mut doomed = BTreeMap::from([("tasks".to_owned(), BTreeSet::from([rowid]))]);
  loop {
    let mut grew = false;
    for reference in &references {
      let Some(parents) = doomed.get(&reference.parent).filter(|rows| !rows.is_empty()) else {
        continue;
      };
      let sql = format!(
        "SELECT rowid FROM \"{table}\" WHERE ({columns}) IN (SELECT {parent_columns} FROM \"{parent}\" WHERE rowid IN \
         (SELECT value FROM json_each(?1)))",
        table = reference.table,
        columns = quoted(&reference.columns),
        parent_columns = quoted(&reference.parent_columns),
        parent = reference.parent,
      );
      let found = connection
        .prepare(&sql)?
        .query_map([json_list(parents)], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
      let rows = doomed.entry(reference.table.clone()).or_default();
      for row in found {
        grew |= rows.insert(row);
      }
    }
    if !grew {
      return Ok(doomed);
    }
  }
}

/// Every foreign key of the schema that deleting a row follows: those that set the reference to NULL keep the row.
fn references(connection: &Connection) -> Result<Vec<Reference>, StoreError> {
  let mut statement = connection.prepare(
    "SELECT m.name, f.id, f.\"table\", f.\"from\", f.\"to\", f.on_delete FROM sqlite_master AS m, \
     pragma_foreign_key_list(m.name) AS f WHERE m.type = 'table' ORDER BY m.name, f.id, f.seq",
  )?;
  let rows = statement
    .query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, i64>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, Option<String>>(4)?,
        row.get::<_, String>(5)?,
      ))
    })?
    .collect::<Result<Vec<_>, _>>()?;
  let mut keys: BTreeMap<(String, i64), Reference> = BTreeMap::new();
  for (table, key, parent, from, to, on_delete) in rows {
    if on_delete.eq_ignore_ascii_case("SET NULL") {
      continue;
    }
    let to = to.ok_or_else(|| corrupt("foreign key", format!("{table}.{from} names no column of {parent}")))?;
    let reference = keys.entry((table.clone(), key)).or_insert_with(|| Reference {
      table,
      columns: Vec::new(),
      parent,
      parent_columns: Vec::new(),
    });
    reference.columns.push(from);
    reference.parent_columns.push(to);
  }
  Ok(keys.into_values().collect())
}

/// Deletes `doomed`, the guards of their tables that refuse every deletion dropped meanwhile and made again as
/// they were, in the same transaction.
fn delete_unguarded(transaction: &Transaction<'_>, doomed: &BTreeMap<String, BTreeSet<i64>>) -> Result<(), StoreError> {
  let mut guards = Vec::new();
  for table in doomed.keys() {
    let mut statement =
      transaction.prepare("SELECT name, sql FROM sqlite_master WHERE type = 'trigger' AND tbl_name = ?1")?;
    let triggers = statement
      .query_map([table], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    guards.extend(
      triggers
        .into_iter()
        .filter(|(_, sql)| sql.to_ascii_uppercase().contains("BEFORE DELETE")),
    );
  }
  for (name, _) in &guards {
    transaction.execute_batch(&format!("DROP TRIGGER \"{name}\""))?;
  }
  for (table, rows) in doomed {
    transaction.execute(
      &format!("DELETE FROM \"{table}\" WHERE rowid IN (SELECT value FROM json_each(?1))"),
      params![json_list(rows)],
    )?;
  }
  for (_, sql) in &guards {
    transaction.execute_batch(sql)?;
  }
  Ok(())
}

fn quoted(columns: &[String]) -> String {
  columns
    .iter()
    .map(|column| format!("\"{column}\""))
    .collect::<Vec<_>>()
    .join(", ")
}

/// `rows` as a JSON array, for `json_each`.
fn json_list(rows: &BTreeSet<i64>) -> String {
  let listed: Vec<String> = rows.iter().map(i64::to_string).collect();
  format!("[{}]", listed.join(","))
}
