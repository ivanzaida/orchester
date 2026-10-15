//! A person's holds on new starts (`queue_holds`), which the runner reads before it starts a first run.

use orchester_core::{
  id::IdempotencyKey,
  task_group::{HoldActor, HoldChange, HoldScope, QueueHolds},
  time::Timestamp,
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

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

  /// Holds new starts in `scope` from `at` as `actor` (`held`), or lets them go on, in one transaction. A person
  /// holding what automation held makes it the person's; automation does not release a hold a person set
  /// ([`HoldChange::PersonOnly`]).
  pub fn set_queue_hold(
    &mut self,
    (scope, actor): (HoldScope, HoldActor),
    held: bool,
    at: Timestamp,
  ) -> Result<HoldChange, StoreError> {
    let scope = match scope {
      HoldScope::Project => "project".to_owned(),
      HoldScope::Group(key) => format!("{GROUP}{key}"),
    };
    let set_by = match actor {
      HoldActor::Person => "person",
      HoldActor::Automation => "automation",
    };
    // Written at once, so no other writer comes between the read and the write.
    let transaction = self
      .connection
      .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let held_by: Option<String> = transaction
      .query_row("SELECT set_by FROM queue_holds WHERE scope = ?1", [&scope], |row| {
        row.get(0)
      })
      .optional()?;
    let change = match (held, held_by.as_deref()) {
      (false, Some("person")) if actor == HoldActor::Automation => HoldChange::PersonOnly,
      (false, Some(_)) => {
        transaction.execute("DELETE FROM queue_holds WHERE scope = ?1", [&scope])?;
        HoldChange::Changed
      }
      (false, None) => HoldChange::Unchanged,
      (true, None) => {
        transaction.execute(
          "INSERT INTO queue_holds (scope, held_at, set_by) VALUES (?1, ?2, ?3)",
          params![scope, at.unix_millis(), set_by],
        )?;
        HoldChange::Changed
      }
      (true, Some("automation")) if actor == HoldActor::Person => {
        transaction.execute("UPDATE queue_holds SET set_by = 'person' WHERE scope = ?1", [&scope])?;
        HoldChange::Changed
      }
      (true, Some(_)) => HoldChange::Unchanged,
    };
    transaction.commit()?;
    Ok(change)
  }
}
