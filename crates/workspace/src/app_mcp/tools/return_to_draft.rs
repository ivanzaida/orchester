//! `return_to_draft`: takes a queued task whose run never started back to Draft and to Backlog, as a card dropped on
//! the board's Backlog column does, through the same workspace function. A task of a reviewed group goes back only
//! when its group holds it for good (after a task of the group failed, at the group's cap, refused by preflight or by
//! the runner, blocked or canceled), as the Tasks selection bar's *Return to drafts* takes it.

use orchester_core::{
  store::TaskStore,
  task::{TaskStatus, organization::ChangeActor},
};
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::wire;
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::Message,
  tasks::{data::ProjectTasks, moves::write::return_to_draft_in, outside::Outside},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ReturnToDraft {
  /// The project's id, or its name.
  pub project: String,
  /// The task's key, e.g. `KON-12`, or its id.
  pub key: String,
}

pub async fn return_to_draft(services: &Services, args: ReturnToDraft) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let tasks = ProjectTasks::load(&open.repo_root, &services.programs)?;
    let lowered = args.key.to_lowercase();
    let task = tasks
      .tasks
      .iter()
      .find(|task| task.key().to_string().to_lowercase() == lowered || task.id().to_string() == args.key)
      .ok_or_else(|| ToolError::not_found(Missing::Task, &args.key))?;
    let key = task.key().to_string();
    if task.status() != TaskStatus::Queued {
      return Err(refused("not_queued", "appMcp.errors.notQueued", &key));
    }
    if tasks.phases.contains_key(&task.id()) {
      return Err(refused("run_recorded", "appMcp.errors.runRecorded", &key));
    }
    // An MCP client's change, as the board records a synthetic drop.
    return_to_draft_in(&open.repo_root, &services.servers, task.id(), ChangeActor::Orchester).map_err(not_changed)?;
    services.changes.record(open.project.id(), Outside::Tasks);
    let task = ProjectDb::open(&open.repo_root)?
      .task(task.id())?
      .ok_or_else(|| ToolError::not_found(Missing::Task, &key))?;
    Ok(json!({
      "task": {
        "key": key,
        "id": task.id().to_string(),
        "submission": wire::submission(task.status()),
      },
    }))
  })
  .await
}

fn refused(code: &str, key: &'static str, task: &str) -> ToolError {
  ToolError::refused(code, vec![Message::new(key).with("task", task.to_owned())])
}

/// Why the board's move did not happen, with a stable code for each refusal it tells apart.
fn not_changed(message: Message) -> ToolError {
  let code = match message.key {
    "tasks.bulk.notHeld" => "not_held",
    "tasks.board.hasRun" => "run_recorded",
    "tasks.conflict.changedMeanwhile" => "changed_meanwhile",
    key if key.starts_with("sharing.") => "shared_server",
    _ => "not_changed",
  };
  ToolError::refused(code, vec![message])
}
