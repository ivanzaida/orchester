//! A team task neither this computer's records nor the team server's last reading hold: one the server has that never
//! ran here and that no conversation watches yet, such as a task the chat made a moment ago. Its runs recorded nothing
//! of it, and the reading ([`super::team_status`]) is made only of what the project's subscriptions watch, so the
//! chat's start and its `watch` ask the team server for it. The answer is the server's task as it stands now, and its
//! identity is recorded here as a start records it, so a watch on it is read with the next reading.

use std::num::NonZeroU32;

use orchester_core::{project::TaskKey, task::identity::TeamTaskRef};
use orchester_store::ProjectDb;

use super::{led_start::Named, tools::ChatScope};
use crate::app_mcp::{self, Services, ToolError, blocking};

/// The team task `named` (its key or its server id) of `scope`'s project as its team server reads it now, with the
/// id this project's records name it by; `None` in a local project and for a local task of a team project. Not found
/// when the server has no such task, and refused as the server's read is when it cannot be asked (signed out,
/// unreachable).
pub(super) async fn asked(services: &Services, scope: &ChatScope, named: &str) -> Result<Option<Named>, ToolError> {
  let Some(read) = app_mcp::team_task(services, &scope.project.to_string(), named).await? else {
    return Ok(None);
  };
  let root = scope.root.clone();
  blocking(move || {
    let mut db = ProjectDb::open(&root)?;
    let (Some(link), Some(number)) = (db.sharing()?, NonZeroU32::new(read.task.number)) else {
      return Ok(None);
    };
    let team = TeamTaskRef {
      server: link.server,
      project: link.project,
      task: read.task.id,
    };
    let id = db.record_team_task(team, read.task.revision)?;
    Ok(Some(Named {
      id,
      key: TaskKey::new(link.key, number).to_string(),
      module: read.task.content.module,
      blocked: read.task.content.blocked_reason,
    }))
  })
  .await
}
