//! The memory area of the local link: the records a prompt gets, searched and chosen in the project database
//! ([`step_memory`], its FTS5 indexes and the project's Memory setting), the unreadable records and the setting.

use std::ops::Deref;

use orchester_core::memory::{MemorySettings, MemoryStore, Recalled, step_memory};
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
use crate::execution::executor::link::{
  LinkError,
  memory::{MemoryLink, RecallRequest, SettingsRequest, UnreadableMemories, UnreadableRequest},
};

impl<D: Deref<Target = ProjectDb>> MemoryLink for LocalLink<D> {
  fn recall(&self, request: &RecallRequest) -> Result<Recalled, LinkError> {
    step_memory(&*self.0, &request.step, request.at).map_err(|error| read_failed(&error))
  }

  fn unreadable(&self, _: &UnreadableRequest) -> Result<UnreadableMemories, LinkError> {
    let records = self.0.unreadable_memories().map_err(|error| read_failed(&error))?;
    Ok(UnreadableMemories { records })
  }

  fn settings(&self, _: &SettingsRequest) -> Result<MemorySettings, LinkError> {
    self.0.memory_settings().map_err(|error| read_failed(&error))
  }
}
