//! `invocation_memories`: the records chosen for each invocation's prompt, those it was given in
//! prompt order and then those the limits left out, each with why it was chosen and its estimated
//! tokens.

use orchester_core::{
  id::InvocationId,
  memory::{Delivery, InjectedMemory, InjectionReason},
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
        reason: InjectionReason::from_name(&reason).ok_or_else(|| corrupt("injection reason", &reason))?,
        tokens,
        delivery: Delivery::from_name(&delivery).ok_or_else(|| corrupt("memory delivery", &delivery))?,
      })
    })
    .collect()
}
