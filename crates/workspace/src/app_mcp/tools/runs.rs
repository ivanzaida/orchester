//! `get_run`, `pause_run`, `resume_run` and `cancel_run`: a run as its task page shows it, its plan decisions
//! with who decided them included, and the run controls of its topbar. Resume continues an interrupted run from its
//! last checkpoint, as the page's Continue does in Resume's place; Resume and Continue take a shared task's claim
//! first.

use std::path::Path;

use orchester_core::{
  id::{ProjectId, RunId},
  run::{
    Plan, Run, RunStatus,
    review::{ReviewRecord, reviews_usage},
  },
  store::{MergeStore, RunStore, TaskStore},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_exec::programs::Programs;
use orchester_ipc::Command;
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Translate, approvals::approval, base_moved, message_waits::message_waits, orchestrated, plans, wire,
  worktree::worktree_state,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  links::{Link, TaskPage},
  sharing::project::WorkClaim,
  tasks::{
    approvals::{ShownApproval, pending},
    delivery::{self, Delivery},
    detail::{RunView, StepView, run_store_error, run_view},
    history,
    plan::PlanReview,
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

/// The run as its task page shows it; its waiting requests in the app's words (`translate`), as `list_approvals`
/// gives them.
pub async fn get_run(services: &Services, args: InRun, translate: &Translate) -> Result<Value, ToolError> {
  let services = services.clone();
  let (mut read, waiting, project) = blocking(move || read_run(&services, &args)).await?;
  read["awaiting"]["approvals"] = waiting
    .iter()
    .map(|shown| approval(shown, project, translate))
    .collect();
  Ok(read)
}

/// The run `args` names as its task page shows it but for its waiting requests, which come apart with its project.
fn read_run(services: &Services, args: &InRun) -> Result<(Value, Vec<ShownApproval>, ProjectId), ToolError> {
  let open = services.project(&args.project)?;
  let (project, root) = (open.project.id(), open.repo_root);
  let db = ProjectDb::open(&root)?;
  let run = find_run(&db, &args.run)?;
  let task = db
    .task(run.task())?
    .ok_or_else(|| ToolError::not_found(Missing::Task, run.task().to_string()))?;
  let conversation = Link::Task(project, task.key().clone(), TaskPage::Conversation).path();
  let waiting: Vec<ShownApproval> = pending(&db, &root)?
    .into_iter()
    .filter(|shown| shown.approval.run == run.id())
    .collect();
  let asked = db.questions(run.id())?;
  let (questions, proposed) = orchestrated::awaiting(&root, &run, (&asked, &conversation))?;
  let messages = message_waits(&db, &run, &asked, Some(&conversation))?;
  let auto_mode = db.run_auto_mode(run.id())?;
  let worktree = worktree_state(&db, &root, run.id())?;
  let merge = merge_state(&db, (&root, &services.programs), &run)?;
  let view = run_view(&db, run)?;
  let plan = PlanReview::of(&view).map(|review| {
    let mut plan = plans::waiting(&review, true);
    plan["link"] = json!(conversation);
    plan
  });
  let decided = decided_plans(&view, &conversation);
  let usage = view.usage();
  // Invocations whose CLI reported input but no output yet: their output is unknown, not 0.
  let output_unreported = view
    .invocations
    .iter()
    .filter(|invocation| invocation.usage.output_unreported())
    .count();
  let reviewer = reviewer_tokens(&view.reviews);
  let run = &view.run;
  let steps: Vec<Value> = view.steps.iter().map(step).collect();
  let unverified: Vec<Value> = view
    .unverified()
    .iter()
    .map(|check| json!({ "check": check.check, "reason": check.reason }))
    .collect();
  let read = json!({
    "id": run.id().to_string(),
    "number": run.number().get(),
    "task": task.key().to_string(),
    "pipeline": { "name": &*view.pipeline, "version": view.version.get() },
    "status": wire::run_status(run.status(), view.runner_failure),
    "unverified": unverified,
    "tokens": {
      "used": run.used().get(),
      "budget": run.budget().map(TokenBudget::get),
      "input": usage.input,
      "cached_input": usage.cached_input,
      "output": usage.output,
      "output_unreported": output_unreported,
      "reviewer": reviewer,
    },
    "time": {
      "active_seconds": view.active_time(Timestamp::now()).as_secs(),
      "limit_seconds": run.max_duration().map(|limit| limit.as_secs()),
    },
    "steps": steps,
    "plans": decided,
    "auto_mode": auto_mode,
    "worktree": worktree,
    "merge": merge,
    "delivery": super::merge_queue::run_delivery(&db, run.id())?,
    "base_moved": base_moved::of_run(&view),
    "awaiting": {
      "plan_decisions": run.awaiting_plan().len(),
      "plan": plan,
      "answers": run.awaiting_answer().len(),
      "questions": questions,
      "messages": messages,
      "orchestrator_proposals": proposed,
    },
    "created_at": wire::time(run.created_at()),
    "finished_at": view.finished_at.map(wire::time),
    "link": Link::Task(project, task.key().clone(), TaskPage::Summary).path(),
  });
  Ok((read, waiting, project))
}

/// Whether `run`'s work was merged, as its task's Summary tells it: by Orchester, with the merge it recorded, or
/// outside Orchester as git in the checkout at `root` proves it (its base branch holds the run's commits); `null`
/// for a run that has not succeeded.
fn merge_state(db: &ProjectDb, (root, programs): (&Path, &Programs), run: &Run) -> Result<Value, ToolError> {
  if let Some(merge) = db.run_merge(run.id())? {
    return Ok(json!({
      "state": "merged",
      "by": "orchester",
      "into": merge.into.to_string(),
      "revision": merge.revision.to_string(),
      "merged_at": wire::time(merge.merged_at),
    }));
  }
  if run.status() != &RunStatus::Succeeded {
    return Ok(Value::Null);
  }
  Ok(match delivery::of_run(programs, root, run) {
    Delivery::MergedOutside(into) => json!({ "state": "merged", "by": "outside", "into": &*into }),
    Delivery::NothingToMerge => json!({ "state": "nothing_to_merge" }),
    Delivery::Waiting => json!({ "state": "not_merged" }),
  })
}

/// The part of a run's tokens auto mode's reviewer spent: how many reviews asked it, how many of them
/// reported no usage it could read (they count nothing), and what the others reported.
fn reviewer_tokens(reviews: &[ReviewRecord]) -> Value {
  let usage = reviews_usage(reviews);
  json!({
    "reviews": reviews.len(),
    "unknown": reviews.iter().filter(|review| review.usage.is_none()).count(),
    "input": usage.input,
    "cached_input": usage.cached_input,
    "output": usage.output,
  })
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
    let status = wire::run_status(run.status(), history::runner_failure(&db, &run)?);
    Ok(json!({ "run": { "id": id.to_string(), "status": status } }))
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

/// The run's decided plans, oldest first, each with its step and who decided it, as the plan cards
/// say it, and `link`, the task's Conversation where the cards are.
fn decided_plans(view: &RunView, link: &str) -> Vec<Value> {
  let mut shown: Vec<&Plan> = view.plans.iter().collect();
  shown.sort_by_key(|plan| (plan.created_at, plan.version));
  shown
    .into_iter()
    .filter_map(|plan| {
      let mut decided = plans::decided(plan)?;
      let name = view
        .steps
        .iter()
        .find(|step| step.key == plan.step)
        .map_or_else(|| plan.step.to_string(), |step| step.name.to_string());
      decided["step"] = json!({ "key": plan.step.to_string(), "name": name });
      decided["link"] = json!(link);
      Some(decided)
    })
    .collect()
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
