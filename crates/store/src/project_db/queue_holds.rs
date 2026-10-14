//! A person's holds on new starts (`queue_holds`), which the runner reads before it starts a first run.

use orchester_core::{
  id::IdempotencyKey,
  task_group::{HoldActor, HoldScope, QueueHolds},
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
    let mut statement = self
      .connection
      .prepare("SELECT scope, set_by FROM queue_holds ORDER BY held_at, scope")?;
    let rows = statement
      .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
      .collect::<Result<Vec<_>, _>>()?;
    let mut holds = QueueHolds::default();
    for (scope, set_by) in rows {
      let held = if scope == "project" {
        holds.project = true;
        HoldScope::Project
      } else {
        let key = scope
          .strip_prefix(GROUP)
          .ok_or_else(|| corrupt("queue hold scope", &scope))?;
        let key = parse::<IdempotencyKey>("queue hold group", key)?;
        holds.groups.push(key);
        HoldScope::Group(key)
      };
      match set_by.as_str() {
        "person" => holds.by_person.push(held),
        "automation" => {}
        other => return Err(corrupt("queue hold actor", other)),
      }
    }
    Ok(holds)
  }

  /// Holds new starts in `scope` from `at` as `actor` (`held`), or lets them go on; returns whether that changed
  /// anything. A person holding what automation held makes it the person's. Whether `actor` may release is the
  /// caller's to check ([`QueueHolds::by_person`]).
  pub fn set_queue_hold(
    &mut self,
    (scope, actor): (HoldScope, HoldActor),
    held: bool,
    at: Timestamp,
  ) -> Result<bool, StoreError> {
    let scope = match scope {
      HoldScope::Project => "project".to_owned(),
      HoldScope::Group(key) => format!("{GROUP}{key}"),
    };
    let changed = if held {
      let set_by = match actor {
        HoldActor::Person => "person",
        HoldActor::Automation => "automation",
      };
      self.connection.execute(
        "INSERT INTO queue_holds (scope, held_at, set_by) VALUES (?1, ?2, ?3)
         ON CONFLICT (scope) DO UPDATE SET set_by = 'person' WHERE excluded.set_by = 'person' AND set_by <> 'person'",
        params![scope, at.unix_millis(), set_by],
      )?
    } else {
      self
        .connection
        .execute("DELETE FROM queue_holds WHERE scope = ?1", [scope])?
    };
    Ok(changed == 1)
  }
}
