//! What the delivery charts and the forecast are built from, read from the project's store and nothing else: each
//! task with when it was made, its workflow status history, when its first agent started and when its work was last
//! merged (in Orchester, or found merged outside it by git: `orchester_store`'s `save_run_merge`), its story points
//! and its attempts; and each step execution that ran, with its active agent time and whether its work was returned.
//! Blocks on SQLite.

use std::{collections::HashMap, sync::Arc};

use orchester_core::{
  id::{ModuleId, StepExecutionId, TaskId},
  run::{AttemptReason, Run},
  store::{ActivityStore, MergeStore, RunStore},
  task::{Task, TaskStatus, organization::WorkflowStatus},
  time::Timestamp,
};
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::detail::run_store_error;

/// One task as delivery counts it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskFacts {
  pub task: TaskId,
  /// `KON-12`.
  pub key: Arc<str>,
  pub title: Arc<str>,
  pub module: Option<ModuleId>,
  pub created_at: Timestamp,
  /// A person submitted it for running: it is in the project's queued work until it is Done or Canceled.
  pub queued: bool,
  /// The workflow status before its first recorded change.
  pub first_status: WorkflowStatus,
  /// Each change of its workflow status, oldest first.
  pub statuses: Vec<(Timestamp, WorkflowStatus)>,
  /// When the first agent or command of its runs started; `None` before any did.
  pub first_start: Option<Timestamp>,
  /// When the work of its latest merged run was merged; `None` while none was.
  pub merged_at: Option<Timestamp>,
  /// Its story points; `None` while it has none.
  pub points: Option<u32>,
  /// Attempts over all its runs.
  pub attempts: u32,
}

impl TaskFacts {
  /// Its workflow status just before `at`.
  pub fn status_before(&self, at: Timestamp) -> WorkflowStatus {
    self
      .statuses
      .iter()
      .rev()
      .find(|(when, _)| *when < at)
      .map_or(self.first_status, |(_, status)| *status)
  }

  /// Its workflow status now.
  pub fn status(&self) -> WorkflowStatus {
    self.statuses.last().map_or(self.first_status, |(_, status)| *status)
  }

  /// Still to be delivered: neither Done nor Canceled.
  pub fn open(&self) -> bool {
    !matches!(self.status(), WorkflowStatus::Done | WorkflowStatus::Canceled)
  }
}

/// One step execution that ran: the step's name in the run's pinned pipeline, when its first launch started, the
/// active agent time of its launches (waits for a person left out), and whether a later attempt returned its work
/// (its failure route, or a person returning it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionFacts {
  pub step: Arc<str>,
  pub started_at: Timestamp,
  pub active_ms: u64,
  pub returned: bool,
}

/// A project's delivery facts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeliveryFacts {
  pub tasks: Vec<TaskFacts>,
  pub executions: Vec<ExecutionFacts>,
  /// How many runs could not be read; their attempts, executions, start and merge are left out.
  pub unread_runs: usize,
}

impl DeliveryFacts {
  /// The facts of `tasks`, the project's in `db`, as they stand at `now`.
  pub fn read(db: &ProjectDb, tasks: &[Task], now: Timestamp) -> Result<Self, StoreError> {
    let mut facts = Self::default();
    for task in tasks {
      let history = db.task_organization_history(task.id())?;
      let mut runs = Vec::new();
      for listed in db.runs_of_task(task.id()).map_err(run_store_error)? {
        match listed {
          Ok(run) => runs.push(run),
          Err(unreadable) => {
            tracing::warn!(run = %unreadable.id, task = %task.id(), "a run's record is damaged; delivery leaves it out");
            facts.unread_runs += 1;
          }
        }
      }
      let mut task_facts = TaskFacts {
        task: task.id(),
        key: task.key().to_string().into(),
        title: task.details().title.as_str().into(),
        module: task.module(),
        created_at: task.created_at(),
        queued: task.status() == TaskStatus::Queued,
        first_status: history
          .first()
          .map_or(task.organization().status(), |change| change.from.status()),
        statuses: history
          .iter()
          .filter(|change| change.from.status() != change.to.status())
          .map(|change| (change.at, change.to.status()))
          .collect(),
        first_start: None,
        merged_at: None,
        points: task.estimate().map(|estimate| estimate.points().get()),
        attempts: 0,
      };
      for run in &runs {
        task_facts.attempts += u32::try_from(run.attempts().len()).unwrap_or(u32::MAX);
        if let Some(merge) = db.run_merge(run.id())? {
          task_facts.merged_at = task_facts.merged_at.max(Some(merge.merged_at));
        }
        let started = facts.add_executions(db, run, now)?;
        task_facts.first_start = earliest(task_facts.first_start, started);
      }
      facts.tasks.push(task_facts);
    }
    Ok(facts)
  }

  /// Adds the executions of `run` that ran; when the first of them started.
  fn add_executions(&mut self, db: &ProjectDb, run: &Run, now: Timestamp) -> Result<Option<Timestamp>, StoreError> {
    let returned = returned(run);
    let mut ran: HashMap<StepExecutionId, (Timestamp, u64)> = HashMap::new();
    for invocation in db.invocations(run.id())? {
      let active = u64::try_from(invocation.active_millis(now)).unwrap_or(0);
      let entry = ran.entry(invocation.execution).or_insert((invocation.started_at, 0));
      entry.0 = entry.0.min(invocation.started_at);
      entry.1 = entry.1.saturating_add(active);
    }
    let mut first = None;
    for attempt in run.attempts() {
      for execution in attempt.executions() {
        let Some((started_at, active_ms)) = ran.get(&execution.id()) else {
          continue;
        };
        first = earliest(first, Some(*started_at));
        let step = run
          .snapshot()
          .body()
          .step(execution.step())
          .map_or_else(|| execution.step().as_str().into(), |step| step.name.as_str().into());
        self.executions.push(ExecutionFacts {
          step,
          started_at: *started_at,
          active_ms: *active_ms,
          returned: returned.contains(&execution.id()),
        });
      }
    }
    Ok(first)
  }
}

/// The executions of `run` whose work a later attempt returned: for each attempt started because a step failed (its
/// failure route) or a person returned the work of a step waiting for them, that step's latest execution before it.
fn returned(run: &Run) -> Vec<StepExecutionId> {
  let attempts = run.attempts();
  let mut returned = Vec::new();
  for (index, attempt) in attempts.iter().enumerate() {
    let failed = match attempt.reason() {
      AttemptReason::Retry { failed_step, .. } | AttemptReason::Returned { failed_step, .. } => failed_step,
      _ => continue,
    };
    let before = attempts[..index]
      .iter()
      .rev()
      .find_map(|earlier| earlier.executions().iter().find(|execution| execution.step() == failed));
    returned.extend(before.map(|execution| execution.id()));
  }
  returned
}

fn earliest(one: Option<Timestamp>, other: Option<Timestamp>) -> Option<Timestamp> {
  match (one, other) {
    (Some(one), Some(other)) => Some(one.min(other)),
    (one, other) => one.or(other),
  }
}
