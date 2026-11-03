//! The tools that read or keep records of the run for its agent (`docs/plan.md`, M12): `get_task` answers the run's
//! task with its approved plans and the person's decisions and notes; `save_memory` keeps a task or project memory
//! record, active or proposed as the project's memory setting says, refused when it holds a secret's value;
//! `report_progress` keeps the step's progress line, the invocation's secret values removed. Each call opens a
//! connection of its own to the project database (the run's task loop owns the runner's) and blocks on it, off the
//! async workers.

use std::{path::PathBuf, sync::Arc};

use orchester_core::{
  command::{SecretVariable, redact_secrets},
  id::{RunId, StepExecutionId, TaskId},
  memory::{
    AgentProvenance, ContainsSecret, Memory, MemoryFilter, MemoryNote, MemoryScope, MemoryStatus, MemoryStore,
    NoteWrite, ScopeFilter, SecretScreen,
  },
  orchester_tools::{TaskView, ToolResult},
  run::StepProgress,
  secret::SecretVault,
  store::{ConversationStore, RunStore, SecretStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

/// The run, task and step execution an invocation's agent works on, and where their records are.
pub(super) struct RunRecords {
  pub(super) root: PathBuf,
  pub(super) run: RunId,
  pub(super) task: TaskId,
  pub(super) execution: StepExecutionId,
  /// The agent of the step, for the memory records it keeps; `None` for a step without an agent profile.
  pub(super) provenance: Option<AgentProvenance>,
  pub(super) vault: Arc<dyn SecretVault>,
}

impl RunRecords {
  fn db(&self) -> Result<ProjectDb, ToolResult> {
    ProjectDb::open(&self.root)
      .map_err(|error| ToolResult::unavailable(&format!("the project database cannot be opened: {error}")))
  }

  /// `get_task`: the run's task as the run started it, its approved plans and the person's decisions and notes.
  pub(super) fn task(&self) -> ToolResult {
    self.read_task().unwrap_or_else(|unavailable| unavailable)
  }

  fn read_task(&self) -> Result<ToolResult, ToolResult> {
    let db = self.db()?;
    let unreadable =
      |error: &dyn std::error::Error| ToolResult::unavailable(&format!("the task cannot be read: {error}"));
    let task = db
      .task_for_run(self.task, self.run)
      .map_err(|error| unreadable(&error))?
      .ok_or_else(|| ToolResult::unavailable("the run's task no longer exists"))?;
    let plans = db.plans(self.run).map_err(|error| unreadable(&error))?;
    let questions = db.questions(self.run).map_err(|error| unreadable(&error))?;
    let messages = db.messages(self.task).map_err(|error| unreadable(&error))?;
    let view = TaskView {
      run: self.run,
      task: &task,
      plans: &plans,
      questions: &questions,
      messages: &messages,
    };
    Ok(ToolResult::shown(view.text()))
  }

  /// `save_memory`: `note` kept in the task's or the project's memory.
  pub(super) fn save_memory(&self, note: &MemoryNote) -> ToolResult {
    self.keep(note).unwrap_or_else(|unavailable| unavailable)
  }

  fn keep(&self, note: &MemoryNote) -> Result<ToolResult, ToolResult> {
    let provenance = self
      .provenance
      .as_ref()
      .ok_or_else(|| ToolResult::unavailable("only an agent step keeps memory"))?;
    let mut db = self.db()?;
    let unreadable =
      |error: &dyn std::error::Error| ToolResult::unavailable(&format!("memory cannot be kept: {error}"));
    let settings = db.memory_settings().map_err(|error| unreadable(&error))?;
    let secrets = db.secrets().map_err(|error| unreadable(&error))?;
    let screen =
      SecretScreen::load(db.project().id(), &secrets, self.vault.as_ref()).map_err(|error| unreadable(&error))?;
    let write = NoteWrite {
      task: self.task,
      provenance,
      settings: &settings,
      screen: &screen,
      now: Timestamp::now(),
    };
    let filter = MemoryFilter {
      scope: match note.scope(self.task) {
        MemoryScope::Task(task) => ScopeFilter::Task(task),
        MemoryScope::Project => ScopeFilter::Project,
      },
      status: None,
      pinned_only: false,
    };
    let mut kept: Option<Memory> = None;
    let changed = db
      .change_memories(&filter, |earlier| -> Result<_, ContainsSecret> {
        let (changes, record) = note.changes(&write, &earlier)?;
        kept = Some(record);
        Ok(changes)
      })
      .map_err(|error| unreadable(&error))?;
    if let Err(contains) = changed {
      return Ok(ToolResult::unavailable(&format!(
        "nothing was kept: the text holds the value of secret `{}`",
        contains.secret
      )));
    }
    let kept = kept.expect("a kept note's record is set when its changes are made");
    let status = match kept.status() {
      MemoryStatus::Active => "active: agents get it from their next start on",
      _ => "proposed: a person approves it before agents get it",
    };
    Ok(ToolResult::shown(format!("kept memory {} ({status})", kept.id())))
  }

  /// `report_progress`: `progress` as the step's line, the invocation's `secrets` removed.
  pub(super) fn report_progress(&self, progress: &StepProgress, secrets: &[SecretVariable]) -> ToolResult {
    let redacted = match StepProgress::new(&redact_secrets(progress.as_str(), secrets)) {
      Ok(redacted) => redacted,
      Err(error) => return ToolResult::unavailable(&error.to_string()),
    };
    let mut db = match self.db() {
      Ok(db) => db,
      Err(unavailable) => return unavailable,
    };
    match db.report_step_progress(self.execution, &redacted, Timestamp::now()) {
      Ok(()) => ToolResult::shown("reported".to_owned()),
      Err(error) => ToolResult::unavailable(&format!("the progress line cannot be kept: {error}")),
    }
  }
}
