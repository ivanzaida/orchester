//! `decide_plan`: a decision on the plan a gated step waits with, as the plan card of the task page
//! takes it (02.11 `x3lVC`): *Approve plan & continue*, which takes no note, or *Request changes*,
//! which needs one. Approving is automation's only while a person lets it answer approvals (*Let
//! automation answer approvals*), as `decide_approval` is; requesting changes is not gated. An approval names the
//! version it is for, so a client approves only a plan it could read in full (`get_run` gives the whole text); the
//! runner refuses a decision once that version no longer waits. The runner records the decision as automation's, and
//! the card says so. Also the waiting plan as `get_run` and `list_approvals` show it, and a decided plan with who
//! decided it.

use orchester_core::{
  id::{ProjectId, RunId},
  run::{Answerer, Plan, PlanDecision, ReviewNote, RunStatus},
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
  links::TaskPage,
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
  /// plan card approves without one; a blank one counts as none.
  #[serde(default)]
  pub note: Option<String>,
  /// The plan version the decision is for, as `get_run` shows it waiting with its whole text: needed
  /// with `approve`, optional with `request_changes`. The decision is refused when another version
  /// waits now.
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
  let asked = asked_version(args.decision, args.version)?;
  let reading = services.clone();
  let (project, review) = blocking(move || {
    // Approving lets the run go on as a person's approval would: only while a person lets automation
    // answer, as for tool requests. Requesting changes only sends the work back to the agent.
    if args.decision == Decision::Approve && !reading.workspace.automation_answers_approvals()? {
      return Err(ToolError::refused(
        "automation_not_allowed",
        vec![Message::new("appMcp.errors.automationNotAllowed")],
      ));
    }
    let project = reading.project(&args.project)?;
    let db = ProjectDb::open(&project.repo_root)?;
    let view = run_view(&db, find_run(&db, &args.run)?)?;
    let review = PlanReview::of(&view).ok_or_else(|| not_waiting(&view))?;
    if let Some(asked) = asked {
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
    by: Answerer::Automation,
  };
  services
    .runners
    .control_run_in(&project, command)
    .await
    .map_err(|error| match error {
      // Decided since it was read, by a person on the card or another client.
      RunRequestError::Rejected(Rejection::PlanNotAwaited) => {
        ToolError::refused(PLAN_DECIDED, vec![Message::new("appMcp.errors.planNoLongerWaits")])
      }
      _ => ToolError::runner(&error, vec![control_message(&error)]),
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

/// The decision the card makes of `choice` and `note`: approving takes no note (a blank one is none),
/// requesting changes needs one that is not blank.
fn plan_decision(choice: Decision, note: Option<&str>) -> Result<PlanDecision, ToolError> {
  let note = note.filter(|note| !note.trim().is_empty());
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

/// The plan version `version` names, which `approve` needs; versions count from 1.
fn asked_version(choice: Decision, version: Option<u32>) -> Result<Option<u32>, ToolError> {
  match (choice, version) {
    (_, Some(0)) => Err(ToolError::refused(
      "invalid_version",
      vec![Message::new("appMcp.errors.planVersionInvalid")],
    )),
    (Decision::Approve, None) => Err(ToolError::refused(
      "version_required",
      vec![Message::new("appMcp.errors.approveNeedsVersion")],
    )),
    (_, version) => Ok(version),
  }
}

/// Why no plan of `view` takes a decision: its run is paused with the plan waiting, its latest plan
/// was decided already, or none waits.
fn not_waiting(view: &RunView) -> ToolError {
  let run = view.run.number().get().to_string();
  let latest = view.plans.iter().max_by_key(|plan| (plan.created_at, plan.version));
  let (code, key) = match (latest.map(Plan::decision), view.run.status()) {
    (Some(Some(PlanDecision::Approved)), _) => (PLAN_DECIDED, "appMcp.errors.planApproved"),
    (Some(Some(PlanDecision::ChangesRequested { .. })), _) => (PLAN_DECIDED, "appMcp.errors.planChangesRequested"),
    (Some(None), RunStatus::Paused(_)) => ("run_paused", "appMcp.errors.planRunPaused"),
    _ => ("no_plan_waiting", "appMcp.errors.noPlanWaiting"),
  };
  let mut reason = Message::new(key).with("run", run);
  if let Some(plan) = latest.filter(|_| code != "no_plan_waiting") {
    let step = view
      .steps
      .iter()
      .find(|step| step.key == plan.step)
      .map_or_else(|| plan.step.to_string(), |step| step.name.to_string());
    reason = reason
      .with("version", plan.version.get().to_string())
      .with("step", step);
  }
  ToolError::refused(code, vec![reason])
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

/// The plan `review` waits on: its version, its step, who proposed it and when, the start of its
/// text, and the decisions on the step's earlier versions; its whole text too when `whole`.
pub fn waiting(review: &PlanReview, whole: bool) -> Value {
  let plan = review.waiting();
  let earlier: Vec<Value> = review.plans().iter().filter_map(decided).collect();
  let mut shown = json!({
    "version": plan.version.get(),
    "step": { "key": plan.step.to_string(), "name": &*review.step },
    "agent": &*review.agent,
    "proposed_at": wire::time(plan.created_at),
    "excerpt": excerpt(&plan.text),
    "earlier": earlier,
  });
  if whole {
    shown["text"] = json!(&plan.text);
  }
  shown
}

/// The decision on `plan`, with its version: `approved`, or `changes_requested` with the note; who
/// decided, the person in the app or automation through this server; and when. `None` while it waits.
pub fn decided(plan: &Plan) -> Option<Value> {
  let review = plan.review.as_ref()?;
  let (decision, note) = match &review.decision {
    PlanDecision::Approved => ("approved", None),
    PlanDecision::ChangesRequested { note } => ("changes_requested", Some(note.as_str())),
  };
  Some(json!({
    "version": plan.version.get(),
    "decision": decision,
    "note": note,
    "decided_by": review.by,
    "decided_at": wire::time(review.at),
  }))
}

/// The plans the running runs of project database `db` of `project` wait on, in run order; only `run`'s when
/// one is named. Each with its excerpt only (plans can be long, and `get_run` gives a plan's whole text) and
/// the link to its card, on its task's Conversation.
pub fn waiting_plans(db: &ProjectDb, project: ProjectId, run: Option<RunId>) -> Result<Vec<Value>, ToolError> {
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
      let mut shown = waiting(&review, false);
      shown["run"] = json!({ "id": view.id().to_string(), "number": view.run.number().get() });
      shown["link"] = json!(
        task
          .as_deref()
          .and_then(|key| wire::task_link(project, key, TaskPage::Conversation))
      );
      shown["task"] = json!(task);
      plans.push(shown);
    }
  }
  Ok(plans)
}

/// The first characters of `text`, trimmed, with an ellipsis when there is more.
pub(super) fn excerpt(text: &str) -> String {
  let text = text.trim();
  match text.char_indices().nth(EXCERPT_CHARS) {
    Some((end, _)) => format!("{}…", text[..end].trim_end()),
    None => text.to_owned(),
  }
}
