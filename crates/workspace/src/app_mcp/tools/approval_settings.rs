//! Approval settings through Orchester's MCP server (`docs/design/approval-modes.md`): `get_approval_settings` reads
//! a project's *Allow Autopilot for agents in this project* and which connection and model review Manual's requests;
//! `set_approval_settings` changes them, as Project settings does; `switch_run_to_manual` switches a run to Manual for
//! itself, as its status line does. Allowing Autopilot is a person's alone, so automation's asking for it is refused;
//! turning it off is always taken. The reviewer approves requests in the person's place, so automation chooses
//! another one only while a person lets automation answer approvals and change agent permissions. Switching a run to
//! Manual narrows what it does without a person: always taken, and never undone.

use orchester_core::{
  connection::{AgentConnection, Connection, moves_to_another_account},
  resource::ModelId,
  run::{
    Answerer,
    review::{
      AutoModeSettings, ReviewerChoice, ReviewerUnavailable, default_review_model, resolve_reviewer, reviewer_kind,
    },
  },
  store::ConnectionStore,
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
  messages::Message,
  project_settings::autopilot_person_only,
  tasks::{
    outside::Outside,
    run_approvals::{RunApprovalChange, apply_in, reviewer_of, run_mode_in, unavailable_key},
  },
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SwitchRunToManual {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetApprovalSettings {
  /// The project's id, or its name.
  pub project: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetApprovalSettings {
  /// The project's id, or its name.
  pub project: String,
  /// *Allow Autopilot for agents in this project*: only `false` is taken from automation, as only a person allows
  /// Autopilot; unchanged when left out.
  pub allow_autopilot: Option<bool>,
  /// The reviewer Manual asks before the person; unchanged when left out.
  pub reviewer: Option<ReviewerArgument>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ReviewerArgument {
  /// An agent connection's id or name: Claude Code, Codex, or a native one on the Anthropic API. Left out:
  /// the default reviewer (the first Claude Code connection with Haiku 4.5).
  pub connection: Option<String>,
  /// A model of that connection; its default when left out.
  pub model: Option<String>,
  /// A reasoning effort the model takes; the connection's default when left out.
  pub effort: Option<String>,
}

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
pub async fn switch_run_to_manual(services: &Services, args: SwitchRunToManual) -> Result<Value, ToolError> {
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
    apply_in(&root, run.id(), RunApprovalChange::SwitchToManual(Answerer::Automation))?;
    services.changes.record(open.project.id(), Outside::Runs);
    let state = run_mode_in(&root, run.id())?;
    Ok(json!({ "run": { "id": run.id().to_string(), "approval_mode": state.mode } }))
  })
  .await
}

pub async fn get_approval_settings(services: &Services, args: GetApprovalSettings) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let db = ProjectDb::open(&services.project(&args.project)?.repo_root)?;
    settings_json(&db)
  })
  .await
}

pub async fn set_approval_settings(services: &Services, args: SetApprovalSettings) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let mut db = ProjectDb::open(&open.repo_root)?;
    let autopilot = match args.allow_autopilot {
      Some(on) => Some(
        db.allow_autopilot()?
          .changed(on, Answerer::Automation)
          .map_err(|_| ToolError::refused("person_only", vec![autopilot_person_only()]))?,
      ),
      None => None,
    };
    let reviewer = match &args.reviewer {
      Some(reviewer) => Some(reviewer_change(&services, &db, reviewer)?),
      None => None,
    };
    if let Some(reviewer) = reviewer {
      // Read and written in one transaction: a person's save in the window cannot slip in between.
      db.update_auto_mode_settings(|_| AutoModeSettings { reviewer })?;
    }
    if let Some(autopilot) = autopilot {
      db.save_allow_autopilot(autopilot)?;
    }
    services.changes.record(open.project.id(), Outside::AutoMode);
    settings_json(&db)
  })
  .await
}

/// The reviewer `argument` asks for, once automation may choose it: only while a person lets automation answer
/// approvals, and, as the reviewer approves requests of every Manual run in the person's place, change agent
/// permissions when it is another one.
fn reviewer_change(
  services: &Services,
  db: &ProjectDb,
  argument: &ReviewerArgument,
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
fn chosen(connections: &[Connection], argument: &ReviewerArgument) -> Result<Option<ReviewerChoice>, ToolError> {
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

/// The project's *Allow Autopilot* and its reviewer, or why it has none.
fn settings_json(db: &ProjectDb) -> Result<Value, ToolError> {
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
  } }))
}

/// The project has no reviewer, for the reason `why`.
fn unavailable(why: ReviewerUnavailable) -> ToolError {
  ToolError::refused("reviewer_unavailable", vec![Message::new(unavailable_key(why))])
}
