//! `decide_plan`: a decision on the plan a gated step waits with, as the plan card of the task page
//! takes it (02.11 `x3lVC`): *Approve plan & continue*, which takes no note, or *Request changes*,
//! which needs one. The decision names the version that waits, and the runner refuses it once that
//! version no longer waits. Also the waiting plan as `get_run` and `list_approvals` show it.

use orchester_core::{
  id::RunId,
  run::{PlanDecision, ReviewNote},
  store::{RunStore, TaskStore},
};
use orchester_ipc::{Command, Rejection};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  runs::{find_run, status_now},
  wire,
};
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  runner::RunRequestError,
  tasks::{
    detail::{RunView, run_store_error, run_view},
    outside::Outside,
    plan::PlanReview,
    run::control_message,
  },
};

/// The refusal of a decision on a plan that was decided already.
const PLAN_DECIDED: &str = "plan_already_decided";

/// How many characters of a waiting plan its excerpt shows.
const EXCERPT_CHARS: usize = 280;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct DecidePlan {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
  pub decision: Decision,
  /// What should change in the plan: needed with `request_changes`. `approve` takes no note, as the
  /// plan card approves without one.
  #[serde(default)]
  pub note: Option<String>,
  /// The plan version the decision is for, as `get_run` shows it waiting; given, the decision is
  /// refused when another version waits now.
  #[serde(default)]
  pub version: Option<u32>,
}

/// The plan card's decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Decision {
  /// *Approve plan & continue*: the step passes and the steps after it may start.
  Approve,
  /// *Request changes*: the step's agent resumes with the note and proposes a revised plan.
  RequestChanges,
}

pub async fn decide_plan(services: &Services, args: DecidePlan) -> Result<Value, ToolError> {
  let decision = plan_decision(args.decision, args.note.as_deref())?;
  let reading = services.clone();
  let (project, review) = blocking(move || {
    let project = reading.project(&args.project)?;
    let db = ProjectDb::open(&project.repo_root)?;
    let view = run_view(&db, find_run(&db, &args.run)?)?;
    let review = PlanReview::of(&view).ok_or_else(|| not_waiting(&view))?;
    if let Some(asked) = args.version {
      same_version(&review, asked)?;
    }
    Ok((project, review))
  })
  .await?;
  let waiting = review.waiting();
  let command = Command::DecidePlan {
    run: review.run,
    execution: waiting.execution,
    version: waiting.version,
    decision,
  };
  services
    .runners
    .control_run_in(&project, command)
    .await
    .map_err(|error| {
      let reasons = vec![control_message(&error)];
      match error {
        // Decided since it was read, by a person on the card or another client.
        RunRequestError::Rejected(Rejection::PlanNotAwaited) => ToolError::refused(PLAN_DECIDED, reasons),
        _ => ToolError::runner(&error, reasons),
      }
    })?;
  // An open task page reads the decision in place.
  services.changes.record(project.project.id(), Outside::Tasks);
  let mut result = status_now(&project, review.run).await?;
  result["plan"] = json!({
    "version": waiting.version.get(),
    "step": { "key": waiting.step.to_string(), "name": &*review.step },
    "decided": decided_name(args.decision),
  });
  Ok(result)
}

/// The decision the card makes of `choice` and `note`: approving takes no note, requesting changes
/// needs one that is not blank.
fn plan_decision(choice: Decision, note: Option<&str>) -> Result<PlanDecision, ToolError> {
  match (choice, note) {
    (Decision::Approve, None) => Ok(PlanDecision::Approved),
    (Decision::Approve, Some(_)) => Err(ToolError::refused(
      "note_not_taken",
      vec![Message::new("appMcp.errors.approveTakesNoNote")],
    )),
    (Decision::RequestChanges, note) => ReviewNote::new(note.unwrap_or_default())
      .map(|note| PlanDecision::ChangesRequested { note })
      .map_err(|_| ToolError::refused("blank_note", vec![Message::new("appMcp.errors.planNoteRequired")])),
  }
}

/// Why no plan of `view` takes a decision: its latest plan was decided already, or none waits.
fn not_waiting(view: &RunView) -> ToolError {
  let latest = view.plans.iter().max_by_key(|plan| (plan.created_at, plan.version));
  let Some((plan, decision)) = latest.and_then(|plan| plan.decision().map(|decision| (plan, decision))) else {
    return ToolError::refused(
      "no_plan_waiting",
      vec![Message::new("appMcp.errors.noPlanWaiting").with("run", view.run.number().get().to_string())],
    );
  };
  let key = match decision {
    PlanDecision::Approved => "appMcp.errors.planApproved",
    PlanDecision::ChangesRequested { .. } => "appMcp.errors.planChangesRequested",
  };
  let step = view
    .steps
    .iter()
    .find(|step| step.key == plan.step)
    .map_or_else(|| plan.step.to_string(), |step| step.name.to_string());
  ToolError::refused(
    PLAN_DECIDED,
    vec![
      Message::new(key)
        .with("version", plan.version.get().to_string())
        .with("step", step),
    ],
  )
}

/// Refused unless plan version `asked` is the one `review` waits on.
fn same_version(review: &PlanReview, asked: u32) -> Result<(), ToolError> {
  let waiting = review.waiting().version.get();
  if waiting == asked {
    return Ok(());
  }
  Err(ToolError::refused(
    "plan_version_changed",
    vec![
      Message::new("appMcp.errors.planVersionChanged")
        .with("asked", asked.to_string())
        .with("version", waiting.to_string())
        .with("step", review.step.to_string()),
    ],
  ))
}

const fn decided_name(decision: Decision) -> &'static str {
  match decision {
    Decision::Approve => "approved",
    Decision::RequestChanges => "changes_requested",
  }
}

/// The plan `review` waits on: its version, its step, who proposed it and when, and the start of
/// its text.
pub fn waiting(review: &PlanReview) -> Value {
  let plan = review.waiting();
  json!({
    "version": plan.version.get(),
    "step": { "key": plan.step.to_string(), "name": &*review.step },
    "agent": &*review.agent,
    "proposed_at": wire::time(plan.created_at),
    "excerpt": excerpt(&plan.text),
  })
}

/// The plans the running runs of project database `db` wait on, in run order; only `run`'s when
/// one is named.
pub fn waiting_plans(db: &ProjectDb, run: Option<RunId>) -> Result<Vec<Value>, ToolError> {
  let mut plans = Vec::new();
  for listed in db.running_runs().map_err(run_store_error)? {
    // An unreadable run takes no decision; `get_run` names why it cannot be read.
    let Ok(found) = listed else { continue };
    if run.is_some_and(|id| id != found.id()) || found.awaiting_plan().is_empty() {
      continue;
    }
    let task = db.task(found.task())?.map(|task| task.key().to_string());
    let view = run_view(db, found)?;
    if let Some(review) = PlanReview::of(&view) {
      let mut shown = waiting(&review);
      shown["run"] = json!({ "id": view.id().to_string(), "number": view.run.number().get() });
      shown["task"] = json!(task);
      plans.push(shown);
    }
  }
  Ok(plans)
}

/// The first characters of `text`, trimmed, with an ellipsis when there is more.
fn excerpt(text: &str) -> String {
  let text = text.trim();
  match text.char_indices().nth(EXCERPT_CHARS) {
    Some((end, _)) => format!("{}…", text[..end].trim_end()),
    None => text.to_owned(),
  }
}
