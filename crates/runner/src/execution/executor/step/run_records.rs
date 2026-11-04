//! The tools that read or keep records of the run for its agent (`docs/plan.md`, M12): `get_task` answers the run's
//! task with its approved plans and the person's decisions and notes; `save_memory` keeps a task or project memory
//! record, active or proposed as the project's memory setting says, refused when it holds a secret's value;
//! `report_progress` keeps the step's progress line, the invocation's secret values removed. Each call opens a
//! connection of its own to the project database (the run's task loop owns the runner's) and blocks on it, off the
//! async workers; the task's messages and the run's questions are asked through the invocation's own coordinator link.
//! A note is screened here and kept by the coordinator under the project's memory setting, which it reads itself,
//! through the invocation's own sink (`docs/architecture/remote-runners.md`, rule 3).

use std::{path::PathBuf, sync::Arc};

use orchester_core::{
  command::{SecretVariable, redact_secrets},
  id::{RunId, StepExecutionId, TaskId},
  memory::{AgentProvenance, MemoryNote, MemoryStatus, SecretScreen},
  orchester_tools::{TaskView, ToolResult},
  run::StepProgress,
  secret::SecretVault,
  store::{RunStore, SecretStore},
  time::Timestamp,
};
use orchester_store::{MemoryAnswer, MemoryRecord, ProjectDb};
use parking_lot::Mutex;

use super::super::{
  link::{CoordinatorLink, messages as message_link, questions as question_link},
  sink::Sink,
};

/// The run, task and step execution an invocation's agent works on, and where their records are.
pub(super) struct RunRecords {
  pub(super) root: PathBuf,
  /// Where the task's messages and the run's questions are asked for: a link of the invocation's own, which its tool
  /// calls share.
  pub(super) link: Mutex<Box<dyn CoordinatorLink + Send>>,
  /// Where the memory records its agent keeps are sent: a sink of the invocation's own.
  pub(super) sink: Mutex<Box<dyn Sink>>,
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
    let questions = question_link::questions(&**self.link.lock(), self.run).map_err(|error| unreadable(&error))?;
    let messages = message_link::messages(&**self.link.lock(), self.task).map_err(|error| unreadable(&error))?;
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
    let db = self.db()?;
    let unreadable =
      |error: &dyn std::error::Error| ToolResult::unavailable(&format!("memory cannot be kept: {error}"));
    let secrets = db.secrets().map_err(|error| unreadable(&error))?;
    let screen =
      SecretScreen::load(db.project().id(), &secrets, self.vault.as_ref()).map_err(|error| unreadable(&error))?;
    if let Err(contains) = note.screen(&screen) {
      return Ok(ToolResult::unavailable(&format!(
        "nothing was kept: the text holds the value of secret `{}`",
        contains.secret
      )));
    }
    // The coordinator reads the scope's records and changes them in one transaction, under the memory setting.
    let record = MemoryRecord::Note {
      note: note.clone(),
      task: self.task,
      provenance: provenance.clone(),
      at: Timestamp::now(),
    };
    let answer = self
      .sink
      .lock()
      .keep_memory(record)
      .map_err(|error| unreadable(&error))?;
    let MemoryAnswer::Note { memory, status } = answer else {
      return Err(ToolResult::unavailable("memory cannot be kept: no note was kept"));
    };
    let status = match status {
      MemoryStatus::Active => "active: agents get it from their next start on",
      _ => "proposed: a person approves it before agents get it",
    };
    Ok(ToolResult::shown(format!("kept memory {memory} ({status})")))
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
