//! Pausing a run, resuming one a pause stopped and letting new starts go on, which the Orchestrator's agent asks for
//! in a turn a wait or a run's end started by itself (`docs/design/orchestrator-toolset.md`, decision 5): nobody asked
//! for them there, so each waits for the person as a proposal, applied through the same tool. Only a run a pause
//! stopped is resumed from the chat: one stopped at a limit or interrupted is the person's.

use std::path::Path;

use orchester_core::{
  id::ProjectId,
  orchestrator::change::{Change, ProposedChange},
  run::{PauseReason, Run, RunLimit, RunStatus},
  task_group::HoldScope,
};
use orchester_store::ProjectDb;
use rmcp::model::JsonObject;
use serde_json::Value;

use super::run_subject;
use crate::{
  app_mcp::{ToolError, find_run, find_task},
  messages::Message,
  tasks::hold,
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
    releasable(&db, task.as_deref(), ToldTo::Agent)?;
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
    resumable(&run, ToldTo::Agent)?;
    Change::Resume { run: id, number }
  };
  Ok((ProposedChange { subject, change }, task))
}

/// Who a refused resume or release is told to.
#[derive(Clone, Copy)]
pub(in crate::orchestrator) enum ToldTo {
  /// The chat's agent, whose call is refused.
  Agent,
  /// The person, whose Apply of the proposal is refused: what it would change changed since, as its words name.
  Applied,
}

/// Refused unless `run` is one the chat resumes: a pause stopped it. One stopped at a limit or interrupted is the
/// person's (`person_only`), told as `to` reads it.
pub(in crate::orchestrator) fn resumable(run: &Run, to: ToldTo) -> Result<(), ToolError> {
  let since = match run.status() {
    RunStatus::Paused(PauseReason::LimitReached(limit)) => match limit {
      RunLimit::Tokens => "orchestrator.proposal.resumeStopped.tokens",
      RunLimit::Time => "orchestrator.proposal.resumeStopped.time",
      RunLimit::DailyBudget => "orchestrator.proposal.resumeStopped.daily",
      RunLimit::GroupBudget => "orchestrator.proposal.resumeStopped.group",
      RunLimit::StepBudget { .. } => "orchestrator.proposal.resumeStopped.step",
      RunLimit::PlanLimit { .. } => "orchestrator.proposal.resumeStopped.plan",
    },
    RunStatus::Interrupted(_) => "orchestrator.proposal.resumeStopped.interrupted",
    RunStatus::Pending
    | RunStatus::Running
    | RunStatus::Paused(PauseReason::Requested)
    | RunStatus::WaitingForPerson(_)
    | RunStatus::Succeeded
    | RunStatus::Failed(_)
    | RunStatus::Cancelled => return Ok(()),
  };
  let key = match to {
    ToldTo::Agent => "orchestrator.tools.resumePersonOnly",
    ToldTo::Applied => since,
  };
  Err(ToolError::refused(
    "person_only",
    vec![Message::new(key).with("run", run.number().get().to_string())],
  ))
}

/// Refused unless automation holds new starts where a release of the chat's lets them go on: in the whole project, or
/// in the reviewed group of task `task` (its key). Nothing held there has nothing to release, and a person's hold is
/// released by a person only, in Tasks; either is told as `to` reads it. Blocks on SQLite.
pub(in crate::orchestrator) fn releasable(db: &ProjectDb, task: Option<&str>, to: ToldTo) -> Result<(), ToolError> {
  let holds = db.queue_holds()?;
  let scope = match task {
    None => Some(HoldScope::Project),
    Some(key) => hold::group_of(db, find_task(db, key)?.id())?.map(HoldScope::Group),
  };
  let held = match scope {
    Some(HoldScope::Project) => holds.project,
    Some(HoldScope::Group(group)) => holds.groups.contains(&group),
    None => false,
  };
  let (code, agent, applied) = match scope {
    _ if !held => (
      "not_held",
      "orchestrator.tools.releaseNothing",
      "orchestrator.proposal.releaseNothing",
    ),
    Some(held) if holds.by_person.contains(&held) => (
      "person_only",
      "orchestrator.tools.releasePersonHeld",
      "orchestrator.proposal.releasePersonHeld",
    ),
    _ => return Ok(()),
  };
  let key = match to {
    ToldTo::Agent => agent,
    ToldTo::Applied => applied,
  };
  Err(ToolError::refused(code, vec![Message::new(key)]))
}

/// Refused when `run` ended since a pause of it was proposed: there is nothing to pause.
pub(super) fn pausable(run: &Run) -> Result<(), ToolError> {
  if !run.status().is_terminal() {
    return Ok(());
  }
  Err(ToolError::refused(
    "run_ended",
    vec![Message::new("orchestrator.proposal.pauseEnded").with("run", run.number().get().to_string())],
  ))
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
