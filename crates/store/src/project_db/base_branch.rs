//! Base branches of tasks and modules (`orchester_core::integration::base`): `tasks.base_choice`/`tasks.base_branch`,
//! `modules.base_branch`, where each run's came from (`runs.base_source`), and the move of the project's base branch
//! to its open modules (`base_branch_move`). Kept on this computer: a shared task's or module's base branch is never
//! sent and leaves its server revision alone, while a local task's raises its revision as any edit does.

use orchester_core::{
  id::{ModuleId, TaskId},
  integration::base::{BaseChoice, RunBaseSource, TaskBase, resolve},
  module::{Module, ModuleError},
  run::BranchName,
  sharing::TaskRevision,
  task::{Task, TaskUpdate},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, params};

use super::{ProjectDb, modules, tasks};
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// The project's base branch as it moved to its open modules that had none (Project settings › General, `NDTCw`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseBranchMove {
  pub branch: BranchName,
  pub at: Timestamp,
  /// The modules that took it, in no order.
  pub modules: Vec<ModuleId>,
}

impl ProjectDb {
  /// The base branch a new run of `task` takes and where it came from: the task's, else its module's as stored now,
  /// else the checked-out branch ([`resolve`]).
  pub fn next_run_base(&self, task: &Task) -> Result<(BaseChoice, RunBaseSource), StoreError> {
    let module = match task.module() {
      Some(id) => modules::load(&self.connection, id)?,
      None => None,
    };
    Ok(resolve(
      task.base(),
      module.as_ref().map(|module| (module.id(), module.base())),
    ))
  }
  /// Sets the base branch of task `id`, read at `read`. A local task changed meanwhile is not changed and comes back
  /// as the conflict; its revision rises as for any edit. A shared task keeps its server revision: its base branch is
  /// this computer's. Runs that started keep the base they recorded.
  pub fn set_task_base(&mut self, id: TaskId, read: TaskRevision, base: &TaskBase) -> Result<TaskUpdate, StoreError> {
    let transaction = self.connection.transaction()?;
    let current = tasks::load_task(&transaction, self.project.key(), id)?;
    let local: bool = transaction.query_row(
      "SELECT sharing IS NULL FROM tasks WHERE id = ?1",
      [id.to_string()],
      |row| row.get(0),
    )?;
    if local && current.revision() != read {
      return Ok(TaskUpdate::Conflict { current });
    }
    let (choice, branch) = task_base_columns(base);
    transaction.execute(
      "UPDATE tasks SET base_choice = ?2, base_branch = ?3, \
       revision = CASE WHEN sharing IS NULL THEN revision + 1 ELSE revision END WHERE id = ?1",
      params![id.to_string(), choice, branch],
    )?;
    let task = tasks::load_task(&transaction, self.project.key(), id)?;
    transaction.commit()?;
    Ok(TaskUpdate::Saved(task))
  }

  /// Sets the base branch of module `id` from `before`, as it was read, to `after`. A base branch that is no longer
  /// `before` is not replaced ([`ModuleError::ChangedMeanwhile`]); a shared module's is this computer's, as a task's.
  pub fn set_module_base(
    &mut self,
    id: ModuleId,
    before: &BaseChoice,
    after: BaseChoice,
  ) -> Result<Module, StoreError> {
    let transaction = self.connection.transaction()?;
    let module = modules::load(&transaction, id)?.ok_or(ModuleError::NotFound(id))?;
    if module.base() != before {
      return Err(ModuleError::ChangedMeanwhile.into());
    }
    transaction.execute(
      "UPDATE modules SET base_branch = ?2 WHERE id = ?1",
      params![id.to_string(), after.branch().map(BranchName::as_str)],
    )?;
    transaction.commit()?;
    Ok(module.with_base(after))
  }

  /// How the project's base branch moved to its open modules; `None` for a project that had none set.
  pub fn base_branch_move(&self) -> Result<Option<BaseBranchMove>, StoreError> {
    let moved: Option<(String, i64)> = self
      .connection
      .query_row("SELECT branch, moved_at FROM base_branch_move", [], |row| {
        Ok((row.get(0)?, row.get(1)?))
      })
      .optional()?;
    let Some((branch, at)) = moved else {
      return Ok(None);
    };
    let mut statement = self
      .connection
      .prepare("SELECT module_id FROM base_branch_move_modules")?;
    let modules = statement
      .query_map([], |row| row.get::<_, String>(0))?
      .map(|id| parse("module id", &id?))
      .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(BaseBranchMove {
      branch: BranchName::new(&branch).map_err(|error| corrupt("moved base branch", error))?,
      at: Timestamp::from_unix_millis(at),
      modules,
    }))
  }

  /// Forgets how the project's base branch moved, once Project settings › General said so and the settings closed:
  /// the line shows once (`NDTCw`).
  pub fn forget_base_branch_move(&mut self) -> Result<(), StoreError> {
    let transaction = self.connection.transaction()?;
    transaction.execute("DELETE FROM base_branch_move_modules", [])?;
    transaction.execute("DELETE FROM base_branch_move", [])?;
    transaction.commit()?;
    Ok(())
  }

  /// How many runs still deliver into the base branch they recorded: those that have not ended, and each task's
  /// latest run that succeeded and that Orchester has not merged. A change of a base branch leaves them on theirs.
  pub fn runs_keeping_their_base(&self) -> Result<usize, StoreError> {
    let count: i64 = self.connection.query_row(
      "SELECT COUNT(*) FROM runs AS run WHERE run.status NOT IN ('succeeded', 'failed', 'cancelled') \
       OR (run.status = 'succeeded' \
         AND NOT EXISTS (SELECT 1 FROM run_merges AS merged WHERE merged.run_id = run.id) \
         AND run.number = (SELECT MAX(other.number) FROM runs AS other WHERE other.task_id = run.task_id))",
      [],
      |row| row.get(0),
    )?;
    usize::try_from(count).map_err(|error| corrupt("run count", error))
  }
}

pub(super) fn task_base_columns(base: &TaskBase) -> (&'static str, Option<&str>) {
  match base {
    TaskBase::Inherit => ("inherit", None),
    TaskBase::CheckedOut => ("checked_out", None),
    TaskBase::Branch(branch) => ("branch", Some(branch.as_str())),
  }
}

pub(super) fn parse_task_base(choice: &str, branch: Option<&str>) -> Result<TaskBase, StoreError> {
  match (choice, branch) {
    ("inherit", None) => Ok(TaskBase::Inherit),
    ("checked_out", None) => Ok(TaskBase::CheckedOut),
    ("branch", Some(branch)) => Ok(TaskBase::Branch(
      BranchName::new(branch).map_err(|error| corrupt("task base branch", error))?,
    )),
    (other, _) => Err(corrupt("task base branch", other)),
  }
}

pub(super) fn parse_module_base(branch: Option<&str>) -> Result<BaseChoice, StoreError> {
  branch.map_or(Ok(BaseChoice::CheckedOut), |branch| {
    BranchName::new(branch)
      .map(BaseChoice::Branch)
      .map_err(|error| corrupt("module base branch", error))
  })
}

pub(super) fn source_columns(source: RunBaseSource) -> (&'static str, Option<String>) {
  match source {
    RunBaseSource::Task => ("task", None),
    RunBaseSource::Module { module } => ("module", Some(module.to_string())),
    RunBaseSource::CheckedOut => ("checked_out", None),
    RunBaseSource::Project => ("project", None),
  }
}

pub(super) fn parse_source(source: &str, module: Option<&str>) -> Result<RunBaseSource, StoreError> {
  match (source, module) {
    ("task", None) => Ok(RunBaseSource::Task),
    ("module", Some(module)) => Ok(RunBaseSource::Module {
      module: parse("run base module", module)?,
    }),
    ("checked_out", None) => Ok(RunBaseSource::CheckedOut),
    ("project", None) => Ok(RunBaseSource::Project),
    (other, _) => Err(corrupt("run base source", other)),
  }
}
