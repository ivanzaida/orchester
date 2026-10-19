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

use super::{super::tools::ChatScope, run_subject};
use crate::{
  app_mcp::{ToolError, blocking, find_run, find_task, not_in_group},
  messages::Message,
  tasks::hold,
};

/// The tools whose calls this module proposes.
pub(in crate::orchestrator) const PAUSE_RUN: &str = "pause_run";
pub(in crate::orchestrator) const RESUME_RUN: &str = "resume_run";
pub(in crate::orchestrator) const RELEASE_QUEUE: &str = "release_queue";

/// What `name` with `arguments` changes in the project at `root` (named `project`), and the key of the task it names;
/// refused as the chat's call of it would be. Blocks on SQLite.
pub(super) fn control_change(
  (root, project): (&Path, &str),
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
    let place = Place {
      project,
      task: task.as_deref(),
    };
    releasable(&db, place, ToldTo::Agent)?;
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

/// Where new starts are held or let go on, as a refusal names it: in project `project` (its name), or in the reviewed
/// group of task `task` (its key).
#[derive(Clone, Copy)]
pub(in crate::orchestrator) struct Place<'a> {
  pub(in crate::orchestrator) project: &'a str,
  pub(in crate::orchestrator) task: Option<&'a str>,
}

/// Refused unless automation holds new starts where a release of the chat's lets them go on, at `place`: in the whole
/// project, or in the reviewed group of its task. A task in no group has no group to release (`not_in_group`), nothing
/// held there has nothing to release (`nothing_held`), and a person's hold is released by a person only, in Tasks
/// (`person_only`); the last two are told as `to` reads them. Blocks on SQLite.
pub(in crate::orchestrator) fn releasable(db: &ProjectDb, place: Place<'_>, to: ToldTo) -> Result<(), ToolError> {
  let holds = db.queue_holds()?;
  let scope = match place.task {
    None => HoldScope::Project,
    Some(named) => {
      let task = find_task(db, named)?;
      let key = task.key().to_string();
      HoldScope::Group(hold::group_of(db, task.id())?.ok_or_else(|| not_in_group(&key))?)
    }
  };
  let held = match scope {
    HoldScope::Project => holds.project,
    HoldScope::Group(group) => holds.groups.contains(&group),
  };
  if !held {
    return Err(nothing_held(place, holds.project, to));
  }
  if !holds.by_person.contains(&scope) {
    return Ok(());
  }
  let key = match to {
    ToldTo::Agent => "orchestrator.tools.releasePersonHeld",
    ToldTo::Applied => "orchestrator.proposal.releasePersonHeld",
  };
  Err(ToolError::refused("person_only", vec![Message::new(key)]))
}

/// A release at `place` where no new starts are held, told as `to` reads it. A group's release while the whole project
/// is held (`project_held`) points to the project's hold.
fn nothing_held(place: Place<'_>, project_held: bool, to: ToldTo) -> ToolError {
  let key = match (to, place.task, project_held) {
    (ToldTo::Agent, None, _) => "orchestrator.tools.releaseNothing.project",
    (ToldTo::Agent, Some(_), false) => "orchestrator.tools.releaseNothing.group",
    (ToldTo::Agent, Some(_), true) => "orchestrator.tools.releaseNothing.projectHeld",
    (ToldTo::Applied, None, _) => "orchestrator.proposal.releaseNothing.project",
    (ToldTo::Applied, Some(_), false) => "orchestrator.proposal.releaseNothing.group",
    (ToldTo::Applied, Some(_), true) => "orchestrator.proposal.releaseNothing.projectHeld",
  };
  let message = Message::new(key)
    .with("project", place.project.to_owned())
    .with("task", place.task.unwrap_or_default().to_owned());
  ToolError::refused("nothing_held", vec![message])
}

/// A hold at `place` where new starts are held already.
fn already_held(place: Place<'_>) -> ToolError {
  let message = match place.task {
    None => Message::new("orchestrator.tools.holdAlready.project").with("project", place.project.to_owned()),
    Some(task) => Message::new("orchestrator.tools.holdAlready.group").with("task", task.to_owned()),
  };
  ToolError::refused("already_held", vec![message])
}

/// A hold or a release of new starts the chat carries out.
#[derive(Clone, Copy)]
pub(in crate::orchestrator) enum QueueCall {
  /// `hold_queue`, only ever the agent's own call: its refusal is told to the agent.
  Hold,
  /// `release_queue`, the agent's own call or a proposal the person applied: its refusal is told as that reads it.
  Release(ToldTo),
}

/// What `call` answered (`told`) in `scope`, for the group of the task `task` names (the whole project when `None`),
/// refused when it changed nothing: new starts held already (`already_held`), or none held to release (`nothing_held`,
/// also when they were let go on between [`releasable`] and the release). Its card then says it was not done instead
/// of showing a hold or a release that did not happen.
pub(in crate::orchestrator) async fn queue_changed(
  told: Value,
  call: QueueCall,
  (scope, task): (&ChatScope, Option<&str>),
) -> Result<Value, ToolError> {
  if told.get("changed").and_then(Value::as_bool) != Some(false) {
    return Ok(told);
  }
  let (root, project, task) = (scope.root.clone(), scope.name.clone(), task.map(str::to_owned));
  blocking(move || -> Result<Value, ToolError> {
    let db = ProjectDb::open(&root)?;
    let key = match task {
      Some(named) => Some(find_task(&db, &named)?.key().to_string()),
      None => None,
    };
    let place = Place {
      project: &project,
      task: key.as_deref(),
    };
    Err(match call {
      QueueCall::Hold => already_held(place),
      QueueCall::Release(to) => nothing_held(place, db.queue_holds()?.project, to),
    })
  })
  .await
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
