//! `list_approvals` and `decide_approval`: the tool requests waiting for a decision, as the approval
//! card shows them, and an answer to one. Automation answers only while a person lets it (*Let
//! automation answer approvals*, off until turned on); it approves once or denies, and the runner
//! records the answer as automation's.

use orchester_core::{
  id::ProjectId,
  run::{Answerer, ApprovalAnswer, Run},
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
  messages::Message,
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
    across(&services, args.project.as_deref(), |open| {
      let db = ProjectDb::open(&open.repo_root)?;
      let run = args.run.as_deref().map(|named| find_run(&db, named)).transpose()?;
      let waiting = pending(&db, &open.repo_root)?;
      let plans = waiting_plans(&db, open.project.id(), run.as_ref().map(Run::id))?;
      let shown = waiting
        .into_iter()
        .filter(|shown| run.as_ref().is_none_or(|run| shown.approval.run == run.id()))
        .collect::<Vec<_>>();
      Ok((shown, plans))
    })
  })
  .await?;
  let mut approvals =
    read.items(|open, (shown, _)| shown.iter().map(|shown| approval(shown, open.project.id())).collect());
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
  let reading = services.clone();
  let (project, request) = blocking(move || {
    if !reading.workspace.automation_answers_approvals()? {
      return Err(ToolError::refused(
        "automation_not_allowed",
        vec![Message::new("appMcp.errors.automationNotAllowed")],
      ));
    }
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
  let answer = match args.decision {
    Decision::ApproveOnce => ApprovalAnswer::Once,
    Decision::Deny => ApprovalAnswer::Deny,
  };
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
  Ok(json!({ "approval": { "id": request.id, "decided": decision_name(args.decision) } }))
}

const fn decision_name(decision: Decision) -> &'static str {
  match decision {
    Decision::ApproveOnce => "approve_once",
    Decision::Deny => "deny",
  }
}

/// A waiting request of `project` as the approval card shows it: what asks, from which run, task, step and
/// agent, since when, and what auto mode did with it, with the link to its card, on its task's Conversation.
/// Recorded arguments have secret values removed already.
pub fn approval(shown: &ShownApproval, project: ProjectId) -> Value {
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
    "link": shown.task.as_deref().and_then(|key| wire::task_link(project, key, TaskPage::Conversation)),
  })
}
