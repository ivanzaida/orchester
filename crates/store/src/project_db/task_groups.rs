//! Atomic first-run queue groups and draft-only pipeline assignment.

use orchester_core::{
  id::{IdempotencyKey, ResourceId, TaskId},
  resource::Pipeline,
  store::{ResourceStore, RunStore, TaskStore},
  task_group::{GroupHold, ReviewedPipeline, Scheduling, TaskGroup},
  task_selection::{self, SelectionProblem},
  tokens::TokensUsed,
};
use rusqlite::{OptionalExtension, params};

use super::{ProjectDb, consistent, tasks::update_task};
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
      return Err(StoreError::PipelineNotPublished(pipeline));
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
        Ok(()) => {
          update_task(&transaction, &task)?;
        }
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
    reviewed: &[ReviewedPipeline],
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
      let pipeline = task.details().pipeline.ok_or(StoreError::TaskNotSelectable {
        task: *id,
        problem: SelectionProblem::Missing(orchester_core::task::MissingDetail::Pipeline),
      })?;
      let reviewed = reviewed
        .iter()
        .find(|entry| entry.task == *id && entry.pipeline == pipeline)
        .ok_or(StoreError::TaskGroupReviewChanged(*id))?;
      let version = self
        .version::<Pipeline>(pipeline, reviewed.version)?
        .ok_or(StoreError::TaskGroupReviewChanged(*id))?;
      let has_run: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM runs WHERE task_id = ?1)",
        [id.to_string()],
        |row| row.get(0),
      )?;
      let blocked_by = super::organization::unfinished(&transaction, *id)?;
      if let Some(problem) = task_selection::problems(&task, has_run, &[pipeline], &blocked_by)
        .into_iter()
        .next()
      {
        return Err(StoreError::TaskNotSelectable { task: *id, problem });
      }
      task
        .queue(version.number())
        .expect("a task without selection problems is a complete draft, which queues");
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

  /// The group of `key` with its members, read together in one consistent read.
  pub fn task_group(&self, key: IdempotencyKey) -> Result<Option<TaskGroup>, StoreError> {
    consistent::read(&self.connection, || self.load_group(key))
  }

  fn load_group(&self, key: IdempotencyKey) -> Result<Option<TaskGroup>, StoreError> {
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
    self.groups_where("SELECT key FROM task_groups ORDER BY rowid")
  }

  /// The groups the runner still schedules: those not [finished](Self::finish_task_group).
  pub fn unfinished_task_groups(&self) -> Result<Vec<TaskGroup>, StoreError> {
    self.groups_where("SELECT key FROM task_groups WHERE finished = 0 ORDER BY rowid")
  }

  /// Records that every member's first run has ended; the group is not scheduled again.
  pub fn finish_task_group(&self, key: IdempotencyKey) -> Result<(), StoreError> {
    self
      .connection
      .execute("UPDATE task_groups SET finished = 1 WHERE key = ?1", [key.to_string()])?;
    Ok(())
  }

  /// Takes held groups' unstarted `tasks` back to the drafts and out of their groups, together:
  /// a failed task or the spent cap holds them for good, and the person decides what runs next.
  /// Their groups keep the members that started; a group left without members is removed.
  pub fn release_held_tasks(&mut self, tasks: &[TaskId]) -> Result<(), StoreError> {
    let transaction = self.connection.unchecked_transaction()?;
    let mut groups = Vec::new();
    for id in tasks {
      let group = self.task_group_for_task(*id)?.ok_or(StoreError::TaskNotHeld(*id))?;
      if !self
        .task_group_hold(group.key())?
        .is_some_and(|hold| hold.releases_unstarted())
      {
        return Err(StoreError::TaskNotHeld(*id));
      }
      let has_run: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM runs WHERE task_id = ?1)",
        [id.to_string()],
        |row| row.get(0),
      )?;
      let mut task = self.task(*id)?.ok_or(StoreError::TaskNotFound(*id))?;
      task
        .return_to_draft(has_run)
        .map_err(|_| StoreError::TaskNotHeld(*id))?;
      transaction.execute("DELETE FROM task_group_members WHERE task_id = ?1", [id.to_string()])?;
      update_task(&transaction, &task)?;
      groups.push(group.key());
    }
    for key in groups {
      transaction.execute(
        "DELETE FROM task_groups WHERE key = ?1 AND NOT EXISTS (SELECT 1 FROM task_group_members WHERE group_key = ?1)",
        [key.to_string()],
      )?;
    }
    transaction.commit()?;
    Ok(())
  }

  /// The groups `query` lists, with their members, in one consistent read. A group removed
  /// meanwhile (its last task taken back to the drafts) is left out.
  fn groups_where(&self, query: &str) -> Result<Vec<TaskGroup>, StoreError> {
    consistent::read(&self.connection, || {
      let mut statement = self.connection.prepare(query)?;
      let keys = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
      let mut groups = Vec::new();
      for key in keys {
        groups.extend(self.load_group(parse("group key", &key)?)?);
      }
      Ok(groups)
    })
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

  /// Scheduling and the shared cap belong to the reviewed first run, not every later run
  /// of its task. Membership is retained for idempotent first-run retries and group history.
  pub fn task_group_for_run(&self, run: &orchester_core::run::Run) -> Result<Option<TaskGroup>, crate::RunStoreError> {
    let first = self.run_with_key(run.task().first_run_key())?;
    if first.is_none_or(|first| first.id() != run.id()) {
      return Ok(None);
    }
    Ok(self.task_group_for_task(run.task())?)
  }

  pub fn task_group_limit_reached(&self, run: &orchester_core::run::Run) -> Result<bool, crate::RunStoreError> {
    let Some(group) = self.task_group_for_run(run)? else {
      return Ok(false);
    };
    Ok(self.task_group_usage(&group)?.reached(group.cap()))
  }
}
