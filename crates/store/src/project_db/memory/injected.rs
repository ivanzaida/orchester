//! `invocation_memories`: the records chosen for each invocation's prompt, those it was given in
//! prompt order and then those the limits left out, each with why it was chosen and its estimated
//! tokens.

use orchester_core::{
  id::{InvocationId, MemoryId},
  memory::{Delivery, GivenMemory, InjectedMemory, InjectionReason},
  time::Timestamp,
};
use rusqlite::{Connection, params};

use crate::{
  StoreError,
  rows::{corrupt, parse},
};

pub(super) fn save(
  connection: &mut Connection,
  invocation: InvocationId,
  injected: &[InjectedMemory],
) -> Result<(), StoreError> {
  let transaction = connection.transaction()?;
  for (position, entry) in injected.iter().enumerate() {
    let changed = transaction.execute(
      "
      INSERT INTO invocation_memories (invocation_id, position, memory_id, reason, tokens, delivery)
      SELECT ?1, ?2, ?3, ?4, ?5, ?6
      WHERE EXISTS (SELECT 1 FROM invocations WHERE id = ?1)",
      params![
        invocation.to_string(),
        i64::try_from(position).map_err(|_| StoreError::Unrepresentable("memory position"))?,
        entry.memory.to_string(),
        entry.reason.name(),
        entry.tokens,
        entry.delivery.name(),
      ],
    )?;
    if changed == 0 {
      return Err(StoreError::InvocationNotFound(invocation));
    }
  }
  transaction.commit()?;
  Ok(())
}

pub(super) fn load(connection: &Connection, invocation: InvocationId) -> Result<Vec<InjectedMemory>, StoreError> {
  let mut statement = connection.prepare(
    "
    SELECT memory_id, reason, tokens, delivery FROM invocation_memories
    WHERE invocation_id = ?1 ORDER BY position",
  )?;
  let rows = statement.query_map([invocation.to_string()], |row| {
    Ok((
      row.get::<_, String>(0)?,
      row.get::<_, String>(1)?,
      row.get::<_, u32>(2)?,
      row.get::<_, String>(3)?,
    ))
  })?;
  rows
    .map(|row| {
      let (memory, reason, tokens, delivery) = row?;
      Ok(InjectedMemory {
        memory: parse("injected memory id", &memory)?,
        reason: reason_value(&reason)?,
        tokens,
        delivery: delivery_value(&delivery)?,
      })
    })
    .collect()
}

/// A row of [`given`]: the invocation, its run and step execution, when it started, why it got the record, its
/// tokens and how it was given.
type GivenRow = (String, String, String, i64, String, u32, String);

/// The invocations given record `memory` whole or cut to fit, latest first.
pub(super) fn given(connection: &Connection, memory: MemoryId) -> Result<Vec<GivenMemory>, StoreError> {
  let mut statement = connection.prepare(
    "
    SELECT i.id, i.run_id, i.step_execution_id, i.started_at, m.reason, m.tokens, m.delivery
    FROM invocation_memories m JOIN invocations i ON i.id = m.invocation_id
    WHERE m.memory_id = ?1 AND m.delivery IN ('given', 'truncated')
    ORDER BY i.started_at DESC, i.id DESC",
  )?;
  let rows = statement.query_map([memory.to_string()], |row| -> rusqlite::Result<GivenRow> {
    Ok((
      row.get(0)?,
      row.get(1)?,
      row.get(2)?,
      row.get(3)?,
      row.get(4)?,
      row.get(5)?,
      row.get(6)?,
    ))
  })?;
  rows
    .map(|row| {
      let (invocation, run, execution, started_at, reason, tokens, delivery) = row?;
      Ok(GivenMemory {
        invocation: parse("invocation id", &invocation)?,
        run: parse("run id", &run)?,
        execution: parse("step execution id", &execution)?,
        started_at: Timestamp::from_unix_millis(started_at),
        reason: reason_value(&reason)?,
        tokens,
        delivery: delivery_value(&delivery)?,
      })
    })
    .collect()
}

fn reason_value(reason: &str) -> Result<InjectionReason, StoreError> {
  InjectionReason::from_name(reason).ok_or_else(|| corrupt("injection reason", reason))
}

fn delivery_value(delivery: &str) -> Result<Delivery, StoreError> {
  Delivery::from_name(delivery).ok_or_else(|| corrupt("memory delivery", delivery))
}
