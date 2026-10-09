//! A record an agent keeps with Orchester's `save_memory` tool (`docs/plan.md`, M12): task memory, or the project's,
//! kept active or proposed as the project's memory setting says for that scope ([`super::Memory::by_agent`]). A note
//! with a key replaces the active record of its scope that has that key: once the note is active it supersedes it,
//! and while it waits for approval the old record stays. A note that holds a secret's value is refused as a whole.

use super::{
  AgentProvenance, AgentWrite, Confidence, ContainsSecret, Memory, MemoryChanges, MemoryContent, MemoryScope,
  MemorySettings, MemoryStatus, SecretScreen,
};
use crate::{id::TaskId, time::Timestamp};

/// Whose memory a note goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteScope {
  /// The run's task.
  Task,
  /// Every task of the project.
  Project,
}

/// What a `save_memory` call asks to keep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryNote {
  scope: NoteScope,
  content: MemoryContent,
}

/// Who keeps a note, for which task, under which setting and screen, when.
#[derive(Clone, Copy)]
pub struct NoteWrite<'a> {
  pub task: TaskId,
  pub provenance: &'a AgentProvenance,
  pub settings: &'a MemorySettings,
  pub screen: &'a SecretScreen,
  pub now: Timestamp,
}

impl MemoryNote {
  pub const fn new(scope: NoteScope, content: MemoryContent) -> Self {
    Self { scope, content }
  }

  /// The memory scope the note goes to in task `task`'s run.
  pub const fn scope(&self, task: TaskId) -> MemoryScope {
    match self.scope {
      NoteScope::Task => MemoryScope::Task(task),
      NoteScope::Project => MemoryScope::Project,
    }
  }

  /// What keeping the note changes, given the records of its scope now (`earlier`), and the note's record as kept.
  pub fn changes(&self, write: &NoteWrite<'_>, earlier: &[Memory]) -> Result<(MemoryChanges, Memory), ContainsSecret> {
    let content = write.screen.check(self.content.clone())?;
    let scope = self.scope(write.task);
    let agent = AgentWrite {
      provenance: write.provenance.clone(),
      confidence: Confidence::Medium,
      sources: Vec::new(),
    };
    let replaced = self.content.key().and_then(|key| {
      earlier.iter().find(|memory| {
        memory.scope() == scope && memory.status() == MemoryStatus::Active && memory.content().key() == Some(key)
      })
    });
    let mut changes = MemoryChanges::default();
    let note = match replaced {
      Some(old) => {
        let mut note =
          Memory::replacing(old, agent, content, write.settings, write.now).expect("the record a key names is active");
        if note.status() == MemoryStatus::Active {
          let mut old = old.clone();
          old
            .supersede(&mut note, write.now)
            .expect("the note is active, of the old record's scope, and names it");
          changes.changed.push(old);
        }
        note
      }
      None => Memory::by_agent(scope, agent, content, write.settings, write.now),
    };
    changes.added.push(note.clone());
    Ok((changes, note))
  }
}
