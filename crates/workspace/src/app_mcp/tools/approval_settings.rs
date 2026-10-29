//! Approvals through Orchester's MCP server (`docs/design/approval-modes.md`): `get_approvals` reads a project's
//! *Allow Autopilot for agents in this project*, which connection and model review Manual's requests, and the mode
//! each run that has not ended answers its agents' requests in, with why; `set_allow_autopilot` turns the project's
//! setting on or off, as Project settings does; `set_reviewer` chooses the reviewer; `set_run_manual` switches a run
//! to Manual for itself, as its status line does. Allowing Autopilot is a person's alone, so automation's asking for
//! it is refused; turning it off is always taken. The reviewer approves requests in the person's place, so automation
//! chooses another one only while a person lets automation answer approvals and change agent permissions. Switching a
//! run to Manual narrows what it does without a person: always taken, and never undone.
//!
//! Auto mode's tools (`get_auto_mode`, `set_auto_mode`, `set_run_auto_mode`) are retired: for one release they stay
//! listed and answer `retired`, naming the tools that replace them ([`retired`]).

use orchester_core::{
  connection::{AgentConnection, Connection, moves_to_another_account},
  id::ProjectId,
  resource::ModelId,
  run::{
    Answerer,
    review::{
      AutoModeSettings, ReviewerChoice, ReviewerUnavailable, default_review_model, resolve_reviewer, reviewer_kind,
    },
  },
  store::{ConnectionStore, RunStore, TaskStore},
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{runs::find_run, setup::widening::allowed};
use crate::{
  agents::form::effort_options,
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  links::{Link, TaskPage},
  messages::Message,
  project_settings::autopilot_person_only,
  tasks::{
    detail::run_store_error,
    outside::Outside,
    run_approvals::{RunApprovalChange, apply_in, reviewer_of, run_mode_in, unavailable_key},
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetRunManual {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetApprovals {
  /// The project's id, or its name.
  pub project: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetAllowAutopilot {
  /// The project's id, or its name.
  pub project: String,
  /// *Allow Autopilot for agents in this project*: `false` is always taken; `true` only while it is on already, as
  /// only a person allows Autopilot.
  pub allow_autopilot: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetReviewer {
  /// The project's id, or its name.
  pub project: String,
  /// An agent connection's id or name: Claude Code, Codex, or a native one on the Anthropic API. Left out:
  /// the default reviewer (the first Claude Code connection with Haiku 4.5).
  pub connection: Option<String>,
  /// A model of that connection; its default when left out.
  pub model: Option<String>,
  /// A reasoning effort the model takes; the connection's default when left out.
  pub effort: Option<String>,
}

/// Whatever a retired tool is given: it answers `retired` alike.
#[derive(Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct Retired {}

/// Refused unless a person lets automation answer approvals.
fn gate(services: &Services) -> Result<(), ToolError> {
  if services.workspace.automation_answers_approvals()? {
    return Ok(());
  }
  Err(ToolError::refused(
    "automation_not_allowed",
    vec![Message::new("appMcp.errors.automationNotAllowed")],
  ))
}

/// Switches a run to Manual for itself: a narrowing, so no switch of the person's gates it.
pub async fn set_run_manual(services: &Services, args: SetRunManual) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let root = open.repo_root;
    let run = find_run(&ProjectDb::open(&root)?, &args.run)?;
    if run.status().is_terminal() {
      return Err(ToolError::refused(
        "run_ended",
        vec![Message::new("runs.control.ended")],
      ));
    }
    apply_in(&root, run.id(), RunApprovalChange::SwitchToManual)?;
    services.changes.record(open.project.id(), Outside::Runs);
    let state = run_mode_in(&root, run.id())?;
    Ok(json!({ "run": {
      "id": run.id().to_string(),
      "number": run.number().get(),
      "approval_mode": state.mode,
    } }))
  })
  .await
}

pub async fn get_approvals(services: &Services, args: GetApprovals) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let db = ProjectDb::open(&open.repo_root)?;
    approvals_json(&db, open.project.id())
  })
  .await
}

pub async fn set_allow_autopilot(services: &Services, args: SetAllowAutopilot) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let autopilot = db
      .allow_autopilot()?
      .changed(args.allow_autopilot, Answerer::Automation)
      .map_err(|_| ToolError::refused("person_only", vec![autopilot_person_only()]))?;
    db.save_allow_autopilot(autopilot)?;
    services.changes.record(open.project.id(), Outside::AutoMode);
    approvals_json(&db, open.project.id())
  })
  .await
}

pub async fn set_reviewer(services: &Services, args: SetReviewer) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let reviewer = reviewer_change(&services, &db, &args)?;
    // Read and written in one transaction: a person's save in the window cannot slip in between.
    db.update_auto_mode_settings(|_| AutoModeSettings { reviewer })?;
    services.changes.record(open.project.id(), Outside::AutoMode);
    approvals_json(&db, open.project.id())
  })
  .await
}

/// A retired tool's answer: refused as `retired`, naming the tools that replace it.
pub fn retired(name: &str) -> ToolError {
  let replacement = match name {
    "get_auto_mode" => "get_approvals",
    "set_auto_mode" => "set_allow_autopilot and set_reviewer",
    _ => "set_run_manual",
  };
  ToolError::refused(
    "retired",
    vec![
      Message::new("appMcp.errors.retired")
        .with("tool", name.to_owned())
        .with("replacement", replacement.to_owned()),
    ],
  )
}

/// The reviewer `argument` asks for, once automation may choose it: only while a person lets automation answer
/// approvals, and, as the reviewer approves requests of every Manual run in the person's place, change agent
/// permissions when it is another one.
fn reviewer_change(
  services: &Services,
  db: &ProjectDb,
  argument: &SetReviewer,
) -> Result<Option<ReviewerChoice>, ToolError> {
  gate(services)?;
  let connections = db.connections()?;
  let reviewer = chosen(&connections, argument)?;
  let current = db.auto_mode_settings()?;
  let next = AutoModeSettings {
    reviewer: reviewer.clone(),
  };
  if next != current {
    let mut reasons = vec![Message::new("appMcp.widens.reviewer")];
    if moves_reviewer_account(&connections, &current, &next) {
      reasons.push(Message::new("appMcp.widens.reviewerAccount"));
    }
    allowed(services, reasons)?;
  }
  Ok(reviewer)
}

/// Whether the reviewer `next` resolves to runs on another account than the one `current` resolves to
/// ([`moves_to_another_account`]): a person's decision, as for an agent profile.
fn moves_reviewer_account(connections: &[Connection], current: &AutoModeSettings, next: &AutoModeSettings) -> bool {
  let account = |settings| {
    resolve_reviewer(settings, connections)
      .ok()
      .map(|resolved| resolved.connection.account().clone())
  };
  account(next).is_some_and(|after| moves_to_another_account(account(current).as_ref(), &after))
}

/// The reviewer `argument` names among `connections`; `None` for the default one.
fn chosen(connections: &[Connection], argument: &SetReviewer) -> Result<Option<ReviewerChoice>, ToolError> {
  let Some(named) = &argument.connection else {
    return Ok(None);
  };
  let connection = connections
    .iter()
    .find(|connection| connection.id().to_string() == *named || connection.name().as_str() == named)
    .ok_or_else(|| ToolError::InvalidArguments(format!("no connection is named {named}")))?;
  if reviewer_kind(connection).is_none() {
    return Err(unavailable(ReviewerUnavailable::Unsupported));
  }
  let model = match &argument.model {
    Some(model) => ModelId::new(model).map_err(|error| ToolError::InvalidArguments(error.to_string()))?,
    None => default_review_model(connection).ok_or_else(|| unavailable(ReviewerUnavailable::NoModel))?,
  };
  if let Some(effort) = &argument.effort {
    let agent =
      AgentConnection::try_from(connection.clone()).map_err(|error| ToolError::InvalidArguments(error.to_string()))?;
    if !effort_options(&agent, &model).contains(effort) {
      return Err(ToolError::InvalidArguments(format!(
        "{} does not take effort {effort}",
        model.as_str()
      )));
    }
  }
  Ok(Some(ReviewerChoice {
    connection: connection.id(),
    model,
    effort: argument.effort.clone(),
  }))
}

/// The project's *Allow Autopilot*, its reviewer or why it has none, and the mode of each of its runs that has not
/// ended, with why it is Manual.
fn approvals_json(db: &ProjectDb, project: ProjectId) -> Result<Value, ToolError> {
  let AutoModeSettings { reviewer } = db.auto_mode_settings()?;
  let resolved = match reviewer_of(db)? {
    Ok(shown) => json!({
      "connection": &*shown.connection,
      "model": shown.model.as_str(),
      "effort": shown.effort,
      "default": reviewer.is_none(),
    }),
    Err(why) => json!({ "unavailable": why }),
  };
  Ok(json!({ "approvals": {
    "allow_autopilot": db.allow_autopilot()?.is_on(),
    "reviewer": resolved,
    "runs": running(db, project)?,
  } }))
}

/// Each run of the project that has not ended, oldest task first, with the mode it answers its agents' requests in
/// now. A run that cannot be read is left out: it cannot be working.
fn running(db: &ProjectDb, project: ProjectId) -> Result<Vec<Value>, ToolError> {
  let mut runs = Vec::new();
  for task in db.tasks()? {
    for run in db.runs_of_task(task.id()).map_err(run_store_error)?.into_iter().flatten() {
      if run.status().is_terminal() {
        continue;
      }
      runs.push(json!({
        "id": run.id().to_string(),
        "number": run.number().get(),
        "task": task.key().to_string(),
        "approval_mode": db.run_approval_mode(run.id())?,
        "link": Link::Task(project, task.key().clone(), TaskPage::Summary).path(),
      }));
    }
  }
  Ok(runs)
}

/// The project has no reviewer, for the reason `why`.
fn unavailable(why: ReviewerUnavailable) -> ToolError {
  ToolError::refused("reviewer_unavailable", vec![Message::new(unavailable_key(why))])
}
