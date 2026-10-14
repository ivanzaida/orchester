//! `hold_queue` and `release_queue`: hold new starts in a project, or in the reviewed group a task belongs to, or
//! let them go on, as Tasks does (`crate::tasks::hold`). Running runs go on either way.

use orchester_core::task_group::HoldScope;
use orchester_store::ProjectDb;
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::tasks::find_task;
use crate::{
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::Message,
  tasks::{hold, outside::Outside},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct QueueHold {
  /// The project's id, or its name.
  pub project: String,
  /// A task of the reviewed group to hold or release, by its key (`KON-12`) or id; left out, the whole project.
  #[serde(default)]
  pub task: Option<String>,
}

/// Holds (`held`) or releases new starts where `args` say; what holds now.
pub async fn set_queue_hold(services: &Services, args: QueueHold, held: bool) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let project = services.project(&args.project)?;
    let scope = match &args.task {
      None => HoldScope::Project,
      Some(named) => {
        let db = ProjectDb::open(&project.repo_root)?;
        let task = find_task(&db, named)?;
        let group = hold::group_of(&db, task.id())?.ok_or_else(|| {
          ToolError::refused(
            "not_in_group",
            vec![Message::new("tasks.queue.notInGroup").with("task", task.key().to_string())],
          )
        })?;
        HoldScope::Group(group)
      }
    };
    let changed = hold::hold_in(&project.repo_root, scope, held).map_err(|message| {
      tracing::warn!(?message, "a hold on new starts could not be kept");
      ToolError::refused("not_saved", vec![message])
    })?;
    services.changes.record(project.project.id(), Outside::Tasks);
    let now = hold::held_in(&ProjectDb::open(&project.repo_root)?)?;
    let groups: Vec<Value> = now.groups.iter().map(|group| json!({ "tasks": group.tasks })).collect();
    Ok(json!({
      "changed": changed,
      "held": { "project": now.project, "groups": groups },
    }))
  })
  .await
}
