//! `hold_queue` and `release_queue`: hold new starts in a project, or in the reviewed group a task belongs to, or
//! let them go on, as Tasks does (`crate::tasks::hold`). Running runs go on either way. These are automation's holds:
//! a hold a person set in the window is the person's to release (`person_only`).

use orchester_core::task_group::{HoldActor, HoldScope};
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
  tasks::{
    hold::{self, HoldRefused},
    outside::Outside,
  },
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
        let group = hold::group_of(&db, task.id())?.ok_or_else(|| not_in_group(&task.key().to_string()))?;
        HoldScope::Group(group)
      }
    };
    let changed =
      hold::hold_in(&project.repo_root, (scope, HoldActor::Automation), held).map_err(|refused| match refused {
        refused @ HoldRefused::PersonOnly => ToolError::refused("person_only", vec![refused.message()]),
        HoldRefused::Store(message) => {
          tracing::warn!(?message, "a hold on new starts could not be kept");
          ToolError::refused("not_saved", vec![message])
        }
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

/// A hold or release named by task `key`, which is in no reviewed group.
pub(crate) fn not_in_group(key: &str) -> ToolError {
  ToolError::refused(
    "not_in_group",
    vec![Message::new("tasks.queue.notInGroup").with("task", key.to_owned())],
  )
}
