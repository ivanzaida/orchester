//! `get_plan`: a plan a gated step of a run proposed, as its card on the task's Conversation shows it: its whole
//! text, whether it waits for a decision or how it was decided and by whom, and the step's other versions. It
//! only reads; deciding a plan is a person's, or `decide_plan`'s.

use orchester_core::{
  id::{ProjectId, StepExecutionId},
  run::{Plan, PlanVersion, Run},
  store::TaskStore,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  plans::{decided, excerpt},
  runs::find_run,
  wire,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  links::{Link, TaskPage},
  messages::Message,
  tasks::{
    detail::{RunView, run_view},
    plan::PlanReview,
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetPlan {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
  /// The gated step's key or name; left out, the step whose plan waits for a decision, else the step of the
  /// run's latest plan.
  #[serde(default)]
  pub step: Option<String>,
  /// The plan's version; left out, the step's latest.
  #[serde(default)]
  pub version: Option<u32>,
}

pub async fn get_plan(services: &Services, args: GetPlan) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    let run = find_run(&db, &args.run)?;
    let link = conversation(&db, &run, open.project.id())?;
    let view = run_view(&db, run)?;
    let versions = step_plans(&view, args.step.as_deref())?;
    let waiting = PlanReview::of(&view).map(|review| (review.waiting().execution, review.waiting().version));
    let plan = match args.version {
      Some(version) => versions
        .iter()
        .rev()
        .find(|plan| plan.version.get() == version)
        .ok_or_else(|| ToolError::not_found(Missing::Plan, version.to_string()))?,
      None => versions.last().expect("a step's plans are never empty"),
    };
    let mut shown = plan_json(&view, plan, waiting);
    shown["text"] = json!(&plan.text);
    shown["versions"] = versions.iter().map(|plan| plan_json(&view, plan, waiting)).collect();
    shown["run"] = json!({ "id": view.id().to_string(), "number": view.run.number().get() });
    shown["link"] = json!(link);
    Ok(json!({ "plan": shown }))
  })
  .await
}

/// The link to `run`'s task's Conversation, where the plan cards are.
fn conversation(db: &ProjectDb, run: &Run, project: ProjectId) -> Result<String, ToolError> {
  let task = db
    .task(run.task())?
    .ok_or_else(|| ToolError::not_found(Missing::Task, run.task().to_string()))?;
  Ok(Link::Task(project, task.key().clone(), TaskPage::Conversation).path())
}

/// The plans of the step `named` (by key or name), or else of the step whose plan the run waits on, or of the
/// latest plan's step; oldest first, never empty. A named step without plans is `plan_not_found`.
fn step_plans<'a>(view: &'a RunView, named: Option<&str>) -> Result<Vec<&'a Plan>, ToolError> {
  let mut plans: Vec<&Plan> = view.plans.iter().collect();
  plans.sort_by_key(|plan| (plan.created_at, plan.version));
  let step = match named {
    Some(named) => {
      let lowered = named.to_lowercase();
      view
        .steps
        .iter()
        .find(|step| step.key.as_str() == named || step.name.to_lowercase() == lowered)
        .map(|step| step.key.clone())
        .ok_or_else(|| ToolError::not_found(Missing::Step, named))?
    }
    // The plan the run waits on, as decide_plan finds it, else the latest.
    None => match PlanReview::of(view) {
      Some(review) => review.waiting().step.clone(),
      None => plans
        .last()
        .map(|plan| plan.step.clone())
        .ok_or_else(|| no_plan(view))?,
    },
  };
  let plans: Vec<&Plan> = plans.into_iter().filter(|plan| plan.step == step).collect();
  if plans.is_empty() {
    return Err(ToolError::not_found(Missing::Plan, step.to_string()));
  }
  Ok(plans)
}

/// A plan of `view` without its text: version, step, agent, when it was proposed, its excerpt, whether it is
/// the plan the run waits on (`waiting`, by its step execution and version) and how it was decided, if it was.
fn plan_json(view: &RunView, plan: &Plan, waiting: Option<(StepExecutionId, PlanVersion)>) -> Value {
  let step = view.steps.iter().find(|step| step.key == plan.step);
  json!({
    "version": plan.version.get(),
    "step": { "key": plan.step.to_string(), "name": step.map(|step| &*step.name) },
    "agent": step.and_then(|step| step.agent.as_deref()),
    "proposed_at": wire::time(plan.created_at),
    "excerpt": excerpt(&plan.text),
    "waiting": waiting == Some((plan.execution, plan.version)),
    "decision": decided(plan),
  })
}

fn no_plan(view: &RunView) -> ToolError {
  ToolError::refused(
    "no_plan",
    vec![Message::new("appMcp.errors.noPlan").with("run", view.run.number().get().to_string())],
  )
}
