//! A person approving a proposed record, with what follows from it: a replacement supersedes the
//! record it names, if that one is still active, and a step's decision or passing handoff makes the
//! latest attempt's decisions the step's current ones, deprecating those of its earlier executions
//! ([`super::handoff`]). A store applies the changes with
//! [`super::MemoryStore::change_memories`], reading the records in the same transaction.

use super::{
  Memory, MemoryAuthor, MemoryChanges, MemoryError, MemoryKind, MemoryStatus,
  handoff::{FAILED_ATTEMPT_TAG, same_step_earlier},
};
use crate::{id::MemoryId, time::Timestamp};

/// What approving proposal `id` changes, given `records`, the records of its scope now.
pub fn approval(id: MemoryId, mut records: Vec<Memory>, now: Timestamp) -> Result<MemoryChanges, MemoryError> {
  let index = records
    .iter()
    .position(|memory| memory.id() == id)
    .ok_or(MemoryError::Unknown { id })?;
  let mut proposal = records.swap_remove(index);
  let replaced = proposal.supersedes().and_then(|replaced| {
    records
      .iter()
      .position(|memory| memory.id() == replaced && memory.status() == MemoryStatus::Active)
  });
  // The replaced record first: it frees its key before the proposal takes it.
  let mut changed = Vec::new();
  match replaced {
    Some(index) => {
      let mut replaced = records.swap_remove(index);
      proposal.approve(Some(&mut replaced), now)?;
      changed.push(replaced);
    }
    None => proposal.approve(None, now)?,
  }
  let steps_decisions = match proposal.author() {
    MemoryAuthor::Agent(agent) if defines_decisions(&proposal) => Some(agent.clone()),
    _ => None,
  };
  changed.push(proposal);
  if let Some(agent) = steps_decisions {
    for mut memory in records {
      if memory.content().kind() == MemoryKind::Decision
        && memory.status() == MemoryStatus::Active
        && same_step_earlier(&memory, &agent)
      {
        memory.deprecate(now).expect("an active record can be deprecated");
        changed.push(memory);
      }
    }
  }
  Ok(MemoryChanges {
    added: Vec::new(),
    changed,
  })
}

/// Whether `memory`, an agent's record, makes its execution's decisions the step's current ones
/// once it is active: a decision, or the handoff of a passing attempt.
fn defines_decisions(memory: &Memory) -> bool {
  let content = memory.content();
  match content.kind() {
    MemoryKind::Decision => true,
    MemoryKind::Handoff => !content.tags().contains(FAILED_ATTEMPT_TAG),
    _ => false,
  }
}
