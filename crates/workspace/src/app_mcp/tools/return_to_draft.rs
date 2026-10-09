//! `return_to_draft`: takes a queued task whose run never started back to Draft, as the Tasks selection
//! bar's *Return to drafts* does: only a task its group holds for good (after a task of the group
//! failed, at the group's cap, refused by preflight or by the runner, blocked or canceled), with the
//! same store rule and refusals.

use std::collections::BTreeSet;

use orchester_core::{store::TaskStore, task::TaskStatus};
use orchester_store::{ProjectDb, StoreError};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::wire;
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  messages::{self, Message},
  tasks::{data::ProjectTasks, group, outside::Outside},
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
    let tasks = ProjectTasks::load(&open.repo_root)?;
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
    if group::releasable(&tasks, &BTreeSet::from([task.id()])).is_empty() {
      return Err(ToolError::refused("not_held", vec![Message::new("tasks.bulk.notHeld")]));
    }
    let mut db = ProjectDb::open(&open.repo_root)?;
    db.release_held_tasks(&[task.id()]).map_err(release_error)?;
    services.changes.record(open.project.id(), Outside::Tasks);
    let task = db
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

/// The store's refusal in the words the selection bar shows it.
fn release_error(error: StoreError) -> ToolError {
  match error {
    StoreError::TaskNotHeld(_) => ToolError::refused("not_held", vec![messages::store_error(&error)]),
    StoreError::SharedTask(_) | StoreError::SharedProject | StoreError::NotSharedTask(_) => {
      ToolError::refused("shared_server", vec![messages::store_error(&error)])
    }
    other => other.into(),
  }
}
