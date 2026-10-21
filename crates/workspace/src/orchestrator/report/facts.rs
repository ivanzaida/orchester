//! What the daily report reads of the project's database, and only that: its tasks with their runs and merges, when
//! each latest run last ended or stopped, the tool requests and questions that wait, the Orchestrator's
//! conversations, the day's usage as `get_usage` reads it, and the daily budget. Blocks on SQLite.

use std::collections::HashMap;

use chrono::NaiveDate;
use orchester_core::{
  account::AgentAccount,
  id::{ChatId, RunId},
  integration::RunMerge,
  orchestrator::ChatEntry,
  question::AskedQuestions,
  run::{PendingToolApproval, Run},
  store::{MergeStore, RunStore, SettingsStore, TaskStore},
  task::Task,
  time::Timestamp,
  usage::{ProjectFacts, day_start},
};
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::detail::run_store_error;

/// A task, its readable runs oldest first, and the merges of those runs.
pub(super) struct TaskFacts {
  pub(super) task: Task,
  pub(super) runs: Vec<Run>,
  pub(super) merges: Vec<RunMerge>,
}

impl TaskFacts {
  pub(super) fn latest(&self) -> Option<&Run> {
    self.runs.last()
  }

  /// Whether `run` was merged in Orchester.
  pub(super) fn merged(&self, run: RunId) -> bool {
    self.merges.iter().any(|merge| merge.run == run)
  }
}

/// Everything the report of `day` is built from, read at `now`.
pub(super) struct DayFacts {
  pub(super) day: NaiveDate,
  pub(super) now: Timestamp,
  /// In the order the project lists them, which is the order they are queued in.
  pub(super) tasks: Vec<TaskFacts>,
  /// When each task's latest run last ended or stopped (succeeded, failed, waits for a person, paused, interrupted).
  pub(super) ended: HashMap<RunId, Timestamp>,
  pub(super) approvals: Vec<PendingToolApproval>,
  pub(super) questions: Vec<AskedQuestions>,
  /// Every conversation of the Orchestrator chat with its entries.
  pub(super) chats: Vec<(ChatId, Vec<ChatEntry>)>,
  pub(super) usage: ProjectFacts,
  pub(super) budget: Option<u64>,
  /// The CLI accounts, which name the accounts of the usage.
  pub(super) accounts: Vec<AgentAccount>,
}

impl DayFacts {
  /// The facts of `day` in `db`, read at `now`, the accounts named by `accounts`.
  pub(super) fn read(
    db: &ProjectDb,
    (day, now): (NaiveDate, Timestamp),
    accounts: Vec<AgentAccount>,
  ) -> Result<Self, StoreError> {
    let mut tasks = Vec::new();
    for task in db.tasks()? {
      // A run whose record cannot be read is left out: nothing it holds can be told.
      let runs: Vec<Run> = db
        .runs_of_task(task.id())
        .map_err(run_store_error)?
        .into_iter()
        .filter_map(Result::ok)
        .collect();
      let mut merges = Vec::new();
      for run in &runs {
        merges.extend(db.run_merge(run.id())?);
      }
      tasks.push(TaskFacts { task, runs, merges });
    }
    let ended = db
      .ended_runs_since(Timestamp::from_unix_millis(0))?
      .into_iter()
      .map(|ended| (ended.run, ended.at))
      .collect();
    let mut chats = Vec::new();
    for listing in db.chats()? {
      let id = listing.chat.id();
      chats.push((id, db.chat_entries(id)?));
    }
    let since = day_start(day);
    Ok(Self {
      day,
      now,
      tasks,
      ended,
      approvals: db.pending_tool_approvals_all()?,
      questions: db.waiting_questions()?,
      chats,
      usage: ProjectFacts {
        project: db.project().id(),
        name: db.project().name().as_str().into(),
        facts: db.usage_facts(since, now)?,
      },
      budget: db.project_limits()?.daily_budget_limit().map(|budget| budget.get()),
      accounts,
    })
  }

  /// The task whose run `run` is, with that run.
  pub(super) fn run(&self, run: RunId) -> Option<(&Task, &Run)> {
    self.tasks.iter().find_map(|facts| {
      facts
        .runs
        .iter()
        .find(|kept| kept.id() == run)
        .map(|kept| (&facts.task, kept))
    })
  }
}
