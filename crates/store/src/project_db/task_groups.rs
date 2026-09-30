//! Atomic first-run queue groups and draft-only pipeline assignment.

use orchester_core::{
  id::{IdempotencyKey, ResourceId, TaskId},
  resource::Pipeline,
  store::{ResourceStore, RunStore, TaskStore},
  task_group::{GroupHold, Scheduling, TaskGroup},
  task_selection::{self, SelectionProblem},
  tokens::TokensUsed,
};
use rusqlite::{OptionalExtension, params};

use super::{ProjectDb, tasks::update_task};
use crate::{
  StoreError,
  rows::{budget_value, corrupt, decode_json, encode_json, parse, tokens_column},
};

impl ProjectDb {
  pub fn task_group_hold(&self, key: IdempotencyKey) -> Result<Option<GroupHold>, StoreError> {
    let hold: Option<String> = self.connection.query_row(
      "SELECT hold FROM task_groups WHERE key = ?1",
      [key.to_string()],
      |row| row.get(0),
    )?;
    hold
      .as_deref()
      .map(|hold| decode_json("group hold", 1, hold))
      .transpose()
  }

  /// Returns whether the persisted reason changed, so the runner only notifies real changes.
  pub fn set_task_group_hold(&self, key: IdempotencyKey, hold: Option<&GroupHold>) -> Result<bool, StoreError> {
    if self.task_group_hold(key)?.as_ref() == hold {
      return Ok(false);
    }
    let encoded = hold.map(|hold| encode_json("group hold", 1, hold)).transpose()?;
    self.connection.execute(
      "UPDATE task_groups SET hold = ?2 WHERE key = ?1",
      params![key.to_string(), encoded],
    )?;
    Ok(true)
  }

  /// Assigns only eligible drafts, returning each skipped task's reason. Publication is checked
  /// again under the write transaction; all eligible changes commit together.
  pub fn set_tasks_pipeline(
    &mut self,
    tasks: &[TaskId],
    pipeline: ResourceId,
  ) -> Result<Vec<(TaskId, SelectionProblem)>, StoreError> {
    let transaction = self.connection.unchecked_transaction()?;
    if self.latest_version::<Pipeline>(pipeline)?.is_none() {
      return Err(StoreError::ResourceNotFound(pipeline));
    }
    let mut skipped = Vec::new();
    for id in tasks {
      let mut task = self.task(*id)?.ok_or(StoreError::TaskNotFound(*id))?;
      let has_run: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM runs WHERE task_id = ?1)",
        [id.to_string()],
        |row| row.get(0),
      )?;
      match task_selection::assign_pipeline(&mut task, has_run, pipeline) {
        Ok(()) => update_task(&transaction, &task)?,
        Err(reason) => skipped.push((*id, reason)),
      }
    }
    transaction.commit()?;
    Ok(skipped)
  }

  /// Persists the reviewed order and queues every draft together. A retry returns the original
  /// group; a different group cannot claim any of these tasks even after a lost reply.
  pub fn queue_task_group(
    &mut self,
    group: &TaskGroup,
    limits: Option<orchester_core::task::TaskLimits>,
  ) -> Result<TaskGroup, StoreError> {
    let transaction = self.connection.unchecked_transaction()?;
    if let Some(existing) = self.task_group(group.key())? {
      return Ok(existing);
    }
    let mut queued = Vec::new();
    for id in group.tasks() {
      let mut task = self.task(*id)?.ok_or(StoreError::TaskNotFound(*id))?;
      if let Some(limits) = limits {
        let mut details = task.details().clone();
        details.limits = Some(limits);
        task
          .edit(details, false)
          .map_err(|source| StoreError::TaskNotEditable { task: *id, source })?;
      }
      let pipeline = task
        .details()
        .pipeline
        .ok_or_else(|| corrupt("task group", "pipeline missing"))?;
      let version = self
        .latest_version::<Pipeline>(pipeline)?
        .ok_or(StoreError::ResourceNotFound(pipeline))?;
      let has_run: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM runs WHERE task_id = ?1)",
        [id.to_string()],
        |row| row.get(0),
      )?;
      let problems = task_selection::problems(&task, has_run, &[pipeline]);
      if !problems.is_empty() {
        return Err(corrupt("task group", format!("{id}: {problems:?}")));
      }
      task
        .queue(version.number())
        .map_err(|error| corrupt("task group", error))?;
      queued.push(task);
    }
    transaction.execute(
      "INSERT INTO task_groups(key, scheduling, cap) VALUES (?1, ?2, ?3)",
      params![
        group.key().to_string(),
        encode_json("scheduling", 1, &group.scheduling())?,
        tokens_column("group cap", group.cap().get())?
      ],
    )?;
    for (position, task) in queued.iter().enumerate() {
      transaction.execute(
        "INSERT INTO task_group_members(group_key, task_id, position) VALUES (?1, ?2, ?3)",
        params![group.key().to_string(), task.id().to_string(), position as i64],
      )?;
      update_task(&transaction, task)?;
    }
    transaction.commit()?;
    Ok(group.clone())
  }

  pub fn task_group(&self, key: IdempotencyKey) -> Result<Option<TaskGroup>, StoreError> {
    let stored: Option<(String, i64)> = self
      .connection
      .query_row(
        "SELECT scheduling, cap FROM task_groups WHERE key = ?1",
        [key.to_string()],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .optional()?;
    let Some((mode, cap)) = stored else {
      return Ok(None);
    };
    let scheduling: Scheduling = decode_json("scheduling", 1, &mode)?;
    let mut statement = self
      .connection
      .prepare("SELECT task_id FROM task_group_members WHERE group_key = ?1 ORDER BY position")?;
    let tasks = statement
      .query_map([key.to_string()], |row| row.get::<_, String>(0))?
      .map(|row| parse("group task", &row?))
      .collect::<Result<Vec<TaskId>, StoreError>>()?;
    TaskGroup::new(key, tasks, scheduling, budget_value("group cap", cap)?)
      .map(Some)
      .map_err(|error| corrupt("task group", error))
  }

  pub fn task_groups(&self) -> Result<Vec<TaskGroup>, StoreError> {
    let mut statement = self.connection.prepare("SELECT key FROM task_groups ORDER BY rowid")?;
    let keys = statement
      .query_map([], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    keys
      .into_iter()
      .map(|key| {
        self
          .task_group(parse("group key", &key)?)?
          .ok_or_else(|| corrupt("task group", "group disappeared"))
      })
      .collect()
  }

  pub fn task_group_usage(&self, group: &TaskGroup) -> Result<TokensUsed, crate::RunStoreError> {
    let mut used = TokensUsed::ZERO;
    for task in group.tasks() {
      if let Some(run) = self.run_with_key(task.first_run_key())? {
        used += run.used();
      }
    }
    Ok(used)
  }

  pub fn task_group_for_task(&self, task: TaskId) -> Result<Option<TaskGroup>, StoreError> {
    let key: Option<String> = self
      .connection
      .query_row(
        "SELECT group_key FROM task_group_members WHERE task_id = ?1",
        [task.to_string()],
        |row| row.get(0),
      )
      .optional()?;
    let Some(key) = key else {
      return Ok(None);
    };
    self.task_group(parse("group key", &key)?)
  }

  pub fn task_group_limit_reached(&self, task: TaskId) -> Result<bool, crate::RunStoreError> {
    let Some(group) = self.task_group_for_task(task)? else {
      return Ok(false);
    };
    Ok(self.task_group_usage(&group)?.reached(group.cap()))
  }
}
