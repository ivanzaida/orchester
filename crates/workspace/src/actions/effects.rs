//! The local effects of a committed team command, exactly those its receipt names. A task the server answers with is
//! recorded as a team task identity, so this computer's records can name it without keeping its definition. Starting,
//! pausing, stopping and delivering work are carried out by P4's execution; until then they stay pending on the open
//! operation, never done in part and never claimed done.

use orchester_api::v2::{LocalEffect, TeamCommandValue};
use orchester_core::task::identity::TeamTaskRef;
use orchester_store::{ProjectDb, StoreError};

use super::ownership::TeamLink;

/// Makes the effects of `value` this computer carries out now, and returns those still pending.
pub(super) fn apply(
  db: &mut ProjectDb,
  link: TeamLink,
  value: &TeamCommandValue,
) -> Result<Vec<LocalEffect>, StoreError> {
  if let TeamCommandValue::Task { task, .. } = value {
    let team = TeamTaskRef {
      server: link.server,
      project: link.project,
      task: task.task.id,
    };
    db.record_team_task(team, task.task.revision)?;
  }
  Ok(
    value
      .effects()
      .iter()
      .filter(|effect| !matches!(effect, LocalEffect::None {}))
      .cloned()
      .collect(),
  )
}
