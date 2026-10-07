//! `get_run`, `pause_run`, `resume_run` and `cancel_run`: a run as its task page shows it, and the run
//! controls of its topbar. Resume continues an interrupted run from its last checkpoint, as the page's
//! Continue does in Resume's place; Resume and Continue take a shared task's claim first.

use orchester_core::{
  id::RunId,
  run::{Run, RunStatus},
  store::{RunStore, TaskStore},
  time::Timestamp,
};
use orchester_ipc::Command;
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{approvals::approval, wire, worktree::worktree_state};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  sharing::project::WorkClaim,
  tasks::{
    approvals::pending,
    detail::{StepView, run_store_error, run_view},
    run::control_message,
  },
  workspace::OpenProject,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InRun {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
}

/// A run control of the task page's topbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
  Pause,
  Resume,
  Cancel,
}

pub async fn get_run(services: &Services, args: InRun) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let root = services.project(&args.project)?.repo_root;
    let db = ProjectDb::open(&root)?;
    let run = find_run(&db, &args.run)?;
    let task = db
      .task(run.task())?
      .ok_or_else(|| ToolError::not_found(Missing::Task, run.task().to_string()))?;
    let waiting: Vec<Value> = pending(&db, &root)?
      .iter()
      .filter(|shown| shown.approval.run == run.id())
      .map(approval)
      .collect();
    let auto_mode = db.run_auto_mode(run.id())?;
    let worktree = worktree_state(&db, &root, run.id())?;
    let view = run_view(&db, run)?;
    let usage = view.usage();
    let run = &view.run;
    let steps: Vec<Value> = view.steps.iter().map(step).collect();
    Ok(json!({
      "id": run.id().to_string(),
      "number": run.number().get(),
      "task": task.key().to_string(),
      "pipeline": { "name": &*view.pipeline, "version": view.version.get() },
      "status": wire::run_status(run.status()),
      "tokens": {
        "used": run.used().get(),
        "budget": run.budget().get(),
        "input": usage.input,
        "cached_input": usage.cached_input,
        "output": usage.output,
      },
      "time": {
        "active_seconds": view.active_time(Timestamp::now()).as_secs(),
        "limit_seconds": run.max_duration().as_secs(),
      },
      "steps": steps,
      "auto_mode": auto_mode,
      "worktree": worktree,
      "awaiting": {
        "plan_decisions": run.awaiting_plan().len(),
        "answers": run.awaiting_answer().len(),
        "approvals": waiting,
      },
      "created_at": wire::time(run.created_at()),
      "finished_at": view.finished_at.map(wire::time),
    }))
  })
  .await
}

pub async fn control_run(services: &Services, args: InRun, control: Control) -> Result<Value, ToolError> {
  let reading = services.clone();
  let (project, run) = blocking(move || {
    let project = reading.project(&args.project)?;
    let run = find_run(&ProjectDb::open(&project.repo_root)?, &args.run)?;
    Ok((project, run))
  })
  .await?;
  let id = run.id();
  // Work on a shared task goes on only under the person's claim; Pause and Cancel need none.
  let claim = match control {
    Control::Resume => services
      .servers
      .claim_for_work(&project.repo_root, run.task())
      .await
      .map_err(|refusal| ToolError::refused("claim_refused", vec![refusal.message()]))?,
    Control::Pause | Control::Cancel => WorkClaim::default(),
  };
  let runners = &services.runners;
  let result = match (control, run.status()) {
    (Control::Pause, _) => runners.control_run_in(&project, Command::PauseRun { run: id }).await,
    (Control::Resume, RunStatus::Interrupted(_)) => runners.continue_run_in(&project, id).await,
    (Control::Resume, _) => runners.control_run_in(&project, Command::ResumeRun { run: id }).await,
    (Control::Cancel, _) => runners.control_run_in(&project, Command::CancelRun { run: id }).await,
  };
  if result.is_err() {
    claim.release(&services.servers).await;
  }
  result.map_err(|error| ToolError::runner(&error, vec![control_message(&error)]))?;
  status_now(&project, id).await
}

/// The run's status as stored once the runner carried out the request.
pub(super) async fn status_now(project: &OpenProject, id: RunId) -> Result<Value, ToolError> {
  let root = project.repo_root.clone();
  blocking(move || {
    let db = ProjectDb::open(&root)?;
    let run = db
      .run(id)
      .map_err(run_store_error)?
      .ok_or_else(|| ToolError::not_found(Missing::Run, id.to_string()))?;
    Ok(json!({ "run": { "id": id.to_string(), "status": wire::run_status(run.status()) } }))
  })
  .await
}

/// The run of project database `db` named by its id or its number (`7` or `#7`).
pub fn find_run(db: &ProjectDb, named: &str) -> Result<Run, ToolError> {
  if let Ok(id) = named.parse::<RunId>() {
    return db
      .run(id)
      .map_err(run_store_error)?
      .ok_or_else(|| ToolError::not_found(Missing::Run, named));
  }
  let number: u32 = named
    .trim_start_matches('#')
    .parse()
    .map_err(|_| ToolError::not_found(Missing::Run, named))?;
  for task in db.tasks()?.iter().chain(&db.not_shared_tasks()?) {
    for listed in db.runs_of_task(task.id()).map_err(run_store_error)? {
      if let Ok(run) = listed
        && run.number().get() == number
      {
        return Ok(run);
      }
    }
  }
  Err(ToolError::not_found(Missing::Run, named))
}

fn step(step: &StepView) -> Value {
  json!({
    "key": step.key.to_string(),
    "name": &*step.name,
    "agent": step.agent.as_deref(),
    "model": step.model.as_deref(),
    "tool_step": step.tool_step,
    "status": step.status.map(wire::step_status),
    "attempt": step.attempt.map(std::num::NonZeroU32::get),
  })
}
