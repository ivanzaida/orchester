//! `list_approvals` and `decide_approval`: the tool requests waiting for a decision, as the approval
//! card shows them, and an answer to one. Automation answers only while a person lets it (*Let
//! automation answer approvals*, off until turned on); it approves once or denies, and the runner
//! records the answer as automation's.

use orchester_core::{
  id::ProjectId,
  run::{Answerer, ApprovalAnswer, DenialNote, Run},
};
use orchester_ipc::Command;
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  Translate,
  across::{across, needs_project, oldest_first},
  plans::waiting_plans,
  runs::find_run,
  wire,
};
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  links::TaskPage,
  tasks::{
    approvals::{ShownApproval, pending},
    run::control_message,
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ListApprovals {
  /// The project's id, or its name; left out, every project the app knows.
  #[serde(default)]
  pub project: Option<String>,
  /// Only the requests of this run: its id, or its number. Needs `project`.
  #[serde(default)]
  pub run: Option<String>,
}

/// The answers automation may give.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Decision {
  /// Approve this request only.
  ApproveOnce,
  /// Deny it: the agent is told and continues.
  Deny,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct DecideApproval {
  /// The project's id, or its name.
  pub project: String,
  /// The request's id, as `list_approvals` gives it.
  pub approval: String,
  pub decision: Decision,
  /// With `deny` only: what the agent is told as the denial's reason, in your words (at most 2000 characters).
  #[serde(default)]
  pub message: Option<String>,
}

pub async fn list_approvals(
  services: &Services,
  args: ListApprovals,
  translate: &Translate,
) -> Result<Value, ToolError> {
  if args.run.is_some() && args.project.is_none() {
    return Err(needs_project("run"));
  }
  let services = services.clone();
  let read = blocking(move || {
    across(&services, args.project.as_deref(), |open, db| {
      let run = args.run.as_deref().map(|named| find_run(db, named)).transpose()?;
      let waiting = pending(db, &open.repo_root)?;
      let plans = waiting_plans(db, open.project.id(), run.as_ref().map(Run::id))?;
      let shown = waiting
        .into_iter()
        .filter(|shown| run.as_ref().is_none_or(|run| shown.approval.run == run.id()))
        .collect::<Vec<_>>();
      Ok((shown, plans))
    })
  })
  .await?;
  let mut approvals = read.items(|open, (shown, _)| {
    shown
      .iter()
      .map(|shown| approval(shown, open.project.id(), translate))
      .collect()
  });
  oldest_first(&mut approvals, "waiting_since");
  let mut plans = read.items(|_, (_, plans)| plans.clone());
  oldest_first(&mut plans, "proposed_at");
  Ok(json!({
    "approvals": approvals,
    "plans": plans,
    "unreadable_projects": read.unreadable(translate),
  }))
}

pub async fn decide_approval(services: &Services, args: DecideApproval) -> Result<Value, ToolError> {
  let answer = answer_of(args.decision, args.message.as_deref())?;
  let decided = decision_name(args.decision);
  let reading = services.clone();
  let (project, request) = blocking(move || {
    crate::decisions::DecisionKind::Approval.require(&reading.workspace)?;
    let project = reading.project(&args.project)?;
    let db = ProjectDb::open(&project.repo_root)?;
    let request = db
      .pending_tool_approvals_all()?
      .into_iter()
      .find(|request| request.id == args.approval)
      .ok_or_else(|| ToolError::not_found(Missing::Approval, &args.approval))?;
    Ok((project, request))
  })
  .await?;
  let message = answer.denial_note().map(|note| note.as_str().to_owned());
  let command = Command::DecideToolApproval {
    run: request.run,
    approval: request.id.clone(),
    answer,
    by: Answerer::Automation,
  };
  services
    .runners
    .control_run_in(&project, command)
    .await
    .map_err(|error| ToolError::runner(&error, vec![control_message(&error)]))?;
  Ok(json!({ "approval": { "id": request.id, "decided": decided, "message": message } }))
}

/// The answer `decision` gives, with `message` as a denial's words: a message only goes with a denial, and must
/// hold words and fit [`DenialNote::MAX_CHARS`].
fn answer_of(decision: Decision, message: Option<&str>) -> Result<ApprovalAnswer, ToolError> {
  match (decision, message) {
    (Decision::ApproveOnce, None) => Ok(ApprovalAnswer::Once),
    (Decision::ApproveOnce, Some(_)) => Err(ToolError::InvalidArguments(
      "`message` goes with `deny` only".to_owned(),
    )),
    (Decision::Deny, None) => Ok(ApprovalAnswer::Deny),
    (Decision::Deny, Some(message)) => DenialNote::new(message)
      .map(|message| ApprovalAnswer::DenyWithMessage { message })
      .map_err(|error| ToolError::InvalidArguments(error.to_string())),
  }
}

const fn decision_name(decision: Decision) -> &'static str {
  match decision {
    Decision::ApproveOnce => "approve_once",
    Decision::Deny => "deny",
  }
}

/// A waiting request of `project` as the approval card shows it: what asks, from which run, task, step and
/// agent, since when, and what auto mode did with it and why it did not review it, in the app's words
/// (`translate`), with the link to its card, on its task's Conversation. Recorded arguments have secret values
/// removed already.
pub fn approval(shown: &ShownApproval, project: ProjectId, translate: &Translate) -> Value {
  let request = &shown.approval;
  let (kind, arguments, reason) = if request.command {
    ("command", None, request.scope.as_deref())
  } else {
    ("mcp_tool", request.scope.as_deref(), None)
  };
  json!({
    "id": &request.id,
    "kind": kind,
    "server": (!request.command).then_some(request.server.as_str()),
    "action": &request.action,
    "arguments": arguments,
    "reason": reason,
    "target": request.target.as_deref(),
    "working_folder": shown.cwd.as_deref(),
    "agent": &request.agent,
    "run": { "id": request.run.to_string(), "number": shown.run.map(|number| number.get()) },
    "task": shown.task.as_deref(),
    "step": shown.step.as_deref(),
    "read_only_step": shown.read_only,
    "waiting_since": wire::time(request.requested_at),
    // What auto mode did with it, and which model reviewed; absent when auto mode did not look at it.
    "auto_mode": shown.review.as_ref(),
    // The hard rule that held it, as the card says it; absent unless a hard rule held it.
    "hold_cause": shown.hold_cause().map(|cause| translate.text(&cause)),
    "link": shown.task.as_deref().and_then(|key| wire::task_link(project, key, TaskPage::Conversation)),
  })
}
