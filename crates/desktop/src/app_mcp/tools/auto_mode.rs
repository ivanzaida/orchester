//! `set_run_auto_mode`: a run's *Auto mode*, as its Approvals panel switches it. Auto mode answers
//! requests in the person's place, so automation changes it only while a person lets automation answer
//! approvals, and only while the project has a Claude Code connection to review with, as the switch is.

use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::runs::find_run;
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  tasks::run_approvals::{RunApprovalChange, apply_in, auto_mode_in},
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

pub async fn set_run_auto_mode(services: &Services, args: SetRunAutoMode) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    if !services.workspace.automation_answers_approvals()? {
      return Err(ToolError::refused(
        "automation_not_allowed",
        vec![Message::new("appMcp.errors.automationNotAllowed")],
      ));
    }
    let root = services.project(&args.project)?.repo_root;
    let run = find_run(&ProjectDb::open(&root)?, &args.run)?;
    if run.status().is_terminal() {
      return Err(ToolError::refused(
        "run_ended",
        vec![Message::new("runs.control.ended")],
      ));
    }
    if !auto_mode_in(&root, run.id())?.reviewer {
      return Err(ToolError::refused(
        "auto_mode_unavailable",
        vec![Message::new("runs.autoMode.unavailable")],
      ));
    }
    apply_in(&root, run.id(), RunApprovalChange::AutoMode(args.enabled))?;
    services.changes.record();
    let state = auto_mode_in(&root, run.id())?;
    Ok(json!({ "run": { "id": run.id().to_string(), "auto_mode": state.on } }))
  })
  .await
}
