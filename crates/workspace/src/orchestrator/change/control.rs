//! Pausing a run, resuming one a pause stopped and letting new starts go on, which the Orchestrator's agent asks for
//! in a turn a wait or a run's end started by itself (`docs/design/orchestrator-toolset.md`, decision 5): nobody asked
//! for them there, so each waits for the person as a proposal, applied through the same tool. Only a run a pause
//! stopped is resumed from the chat: one stopped at a limit or interrupted is the person's.

use std::path::Path;

use orchester_core::{
  id::ProjectId,
  orchestrator::change::{Change, ProposedChange},
  run::{PauseReason, Run, RunStatus},
};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::Value;

use super::run_subject;
use crate::{
  app_mcp::{ToolError, find_run, find_task},
  messages::Message,
};

/// The tools whose calls this module proposes.
pub(in crate::orchestrator) const PAUSE_RUN: &str = "pause_run";
pub(in crate::orchestrator) const RESUME_RUN: &str = "resume_run";
pub(in crate::orchestrator) const RELEASE_QUEUE: &str = "release_queue";

/// What `name` with `arguments` changes in the project at `root`, and the key of the task it names; refused as the
/// chat's call of it would be. Blocks on SQLite.
pub(super) fn control_change(
  root: &Path,
  name: &str,
  arguments: &JsonObject,
) -> Result<(ProposedChange, Option<String>), ToolError> {
  let db = ProjectDb::open(root)?;
  let text = |argument: &str| arguments.get(argument).and_then(Value::as_str).map(str::to_owned);
  if name == RELEASE_QUEUE {
    let task = match text("task") {
      Some(named) => Some(find_task(&db, &named)?.key().to_string()),
      None => None,
    };
    let change = Change::Release { task: task.clone() };
    return Ok((
      ProposedChange {
        subject: String::new(),
        change,
      },
      task,
    ));
  }
  let named = text("run").ok_or_else(|| ToolError::InvalidArguments("run: required".to_owned()))?;
  let run = find_run(&db, &named)?;
  let (subject, task) = run_subject(&db, &run)?;
  let (id, number) = (run.id(), run.number().get());
  let change = if name == PAUSE_RUN {
    Change::Pause { run: id, number }
  } else {
    resumable(&run)?;
    Change::Resume { run: id, number }
  };
  Ok((ProposedChange { subject, change }, task))
}

/// Refused unless `run` is one the chat resumes: a pause stopped it. One stopped at a limit or interrupted is the
/// person's (`person_only`).
pub(in crate::orchestrator) fn resumable(run: &Run) -> Result<(), ToolError> {
  match run.status() {
    RunStatus::Paused(PauseReason::LimitReached(_)) | RunStatus::Interrupted(_) => Err(ToolError::refused(
      "person_only",
      vec![Message::new("orchestrator.tools.resumePersonOnly").with("run", run.number().get().to_string())],
    )),
    _ => Ok(()),
  }
}

/// `release_queue` and its arguments: new starts go on in project `project`, in the reviewed group of task `task` when
/// it names one.
pub(super) fn release_arguments(project: ProjectId, task: Option<&str>) -> (&'static str, JsonObject) {
  let mut arguments = JsonObject::new();
  arguments.insert("project".to_owned(), Value::String(project.to_string()));
  if let Some(task) = task {
    arguments.insert("task".to_owned(), Value::String(task.to_owned()));
  }
  (RELEASE_QUEUE, arguments)
}
