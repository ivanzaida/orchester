//! A person's holds on new starts (`queue_holds`), which the runner reads before it starts a first run.

use orchester_core::{
  id::IdempotencyKey,
  task_group::{HoldScope, QueueHolds},
  time::Timestamp,
};
use rusqlite::params;

use super::ProjectDb;
use crate::{
  StoreError,
  rows::{corrupt, parse},
};

/// The prefix of a group's scope.
const GROUP: &str = "group:";

impl ProjectDb {
  /// The holds on new starts.
  pub fn queue_holds(&self) -> Result<QueueHolds, StoreError> {
    let mut statement = self.connection.prepare("SELECT scope FROM queue_holds ORDER BY held_at, scope")?;
    let scopes = statement
      .query_map([], |row| row.get::<_, String>(0))?
      .collect::<Result<Vec<_>, _>>()?;
    let mut holds = QueueHolds::default();
    for scope in scopes {
      if scope == "project" {
        holds.project = true;
      } else {
        let key = scope
          .strip_prefix(GROUP)
          .ok_or_else(|| corrupt("queue hold scope", &scope))?;
        holds.groups.push(parse::<IdempotencyKey>("queue hold group", key)?);
      }
    }
    Ok(holds)
  }

  /// Holds new starts in `scope` from `at` (`held`), or lets them go on; returns whether that changed anything.
  pub fn set_queue_hold(&mut self, scope: HoldScope, held: bool, at: Timestamp) -> Result<bool, StoreError> {
    let scope = match scope {
      HoldScope::Project => "project".to_owned(),
      HoldScope::Group(key) => format!("{GROUP}{key}"),
    };
    let changed = if held {
      self.connection.execute(
        "INSERT INTO queue_holds (scope, held_at) VALUES (?1, ?2) ON CONFLICT (scope) DO NOTHING",
        params![scope, at.unix_millis()],
      )?
    } else {
      self
        .connection
        .execute("DELETE FROM queue_holds WHERE scope = ?1", [scope])?
    };
    Ok(changed == 1)
  }
}
