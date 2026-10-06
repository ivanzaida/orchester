//! Auto mode through Orchester's MCP server: `get_auto_mode` reads a project's settings and which
//! connection and model review; `set_auto_mode` changes *New runs use auto mode* and the reviewer, as
//! Project settings does; `set_run_auto_mode` switches a run's auto mode, as its Approvals panel does.
//! Auto mode answers requests in the person's place, so automation changes any of it only while a person
//! lets automation answer approvals, and a run's switch only while the project has a reviewer.

use orchester_core::{
  connection::{AgentConnection, Connection},
  resource::ModelId,
  run::review::{AutoModeSettings, ReviewerChoice, ReviewerUnavailable, default_review_model, reviewer_kind},
  store::ConnectionStore,
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::runs::find_run;
use crate::{
  agents::form::effort_options,
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  tasks::run_approvals::{RunApprovalChange, apply_in, auto_mode_in, reviewer_of, unavailable_key},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetRunAutoMode {
  /// The project's id, or its name.
  pub project: String,
  /// The run's id, or its number (`7` or `#7`).
  pub run: String,
  /// Whether a reviewer model answers the run's requests the rules leave to the person.
  pub enabled: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetAutoMode {
  /// The project's id, or its name.
  pub project: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct SetAutoMode {
  /// The project's id, or its name.
  pub project: String,
  /// Whether runs created from now on use auto mode; unchanged when left out.
  pub new_runs: Option<bool>,
  /// The reviewer; unchanged when left out.
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

pub async fn set_run_auto_mode(services: &Services, args: SetRunAutoMode) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    gate(&services)?;
    let root = services.project(&args.project)?.repo_root;
    let run = find_run(&ProjectDb::open(&root)?, &args.run)?;
    if run.status().is_terminal() {
      return Err(ToolError::refused(
        "run_ended",
        vec![Message::new("runs.control.ended")],
      ));
    }
    if let Err(why) = auto_mode_in(&root, run.id())?.reviewer {
      return Err(unavailable(why));
    }
    apply_in(&root, run.id(), RunApprovalChange::AutoMode(args.enabled))?;
    services.changes.record();
    let state = auto_mode_in(&root, run.id())?;
    Ok(json!({ "run": { "id": run.id().to_string(), "auto_mode": state.on } }))
  })
  .await
}

pub async fn get_auto_mode(services: &Services, args: GetAutoMode) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let db = ProjectDb::open(&services.project(&args.project)?.repo_root)?;
    settings_json(&db)
  })
  .await
}

pub async fn set_auto_mode(services: &Services, args: SetAutoMode) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    gate(&services)?;
    let mut db = ProjectDb::open(&services.project(&args.project)?.repo_root)?;
    let mut settings = db.auto_mode_settings()?;
    if let Some(new_runs) = args.new_runs {
      settings.new_runs = new_runs;
    }
    if let Some(reviewer) = &args.reviewer {
      settings.reviewer = chosen(&db.connections()?, reviewer)?;
    }
    db.save_auto_mode_settings(&settings)?;
    services.changes.record();
    settings_json(&db)
  })
  .await
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

/// The project's auto mode settings and its reviewer, or why it has none.
fn settings_json(db: &ProjectDb) -> Result<Value, ToolError> {
  let AutoModeSettings { new_runs, reviewer } = db.auto_mode_settings()?;
  let resolved = match reviewer_of(db)? {
    Ok(shown) => json!({
      "connection": &*shown.connection,
      "model": shown.model.as_str(),
      "effort": shown.effort,
      "default": reviewer.is_none(),
    }),
    Err(why) => json!({ "unavailable": why }),
  };
  Ok(json!({ "auto_mode": { "new_runs": new_runs, "reviewer": resolved } }))
}

/// Auto mode has no reviewer, for the reason `why`.
fn unavailable(why: ReviewerUnavailable) -> ToolError {
  ToolError::refused("auto_mode_unavailable", vec![Message::new(unavailable_key(why))])
}
