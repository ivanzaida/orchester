//! The memory area of the local link: the records a prompt gets, searched and chosen in the project database
//! ([`step_memory`], its FTS5 indexes and the project's Memory setting), the unreadable records and the setting. In
//! the runner's own tests each request and answer crosses as JSON (`executor::link::sent`).

use std::ops::Deref;

use orchester_core::memory::{MemorySettings, MemoryStore, Recalled, step_memory};
use orchester_store::ProjectDb;

use super::{LocalLink, read_failed};
#[cfg(feature = "test-agent")]
use crate::execution::executor::link::sent;
use crate::execution::executor::link::{
  LinkError,
  memory::{MemoryLink, RecallRequest, SettingsRequest, UnreadableMemories, UnreadableRequest},
};

impl<D: Deref<Target = ProjectDb>> MemoryLink for LocalLink<D> {
  fn recall(&self, request: &RecallRequest) -> Result<Recalled, LinkError> {
    #[cfg(feature = "test-agent")]
    sent::sending(request);
    let answer = step_memory(&*self.0, &request.step, request.at).map_err(|error| read_failed(&error));
    #[cfg(feature = "test-agent")]
    sent::sending(&answer);
    answer
  }

  fn unreadable(&self, request: &UnreadableRequest) -> Result<UnreadableMemories, LinkError> {
    #[cfg(feature = "test-agent")]
    sent::sending(request);
    #[cfg(not(feature = "test-agent"))]
    let _ = request;
    let answer = self
      .0
      .unreadable_memories()
      .map(|records| UnreadableMemories { records })
      .map_err(|error| read_failed(&error));
    #[cfg(feature = "test-agent")]
    sent::sending(&answer);
    answer
  }

  fn settings(&self, request: &SettingsRequest) -> Result<MemorySettings, LinkError> {
    #[cfg(feature = "test-agent")]
    sent::sending(request);
    #[cfg(not(feature = "test-agent"))]
    let _ = request;
    let answer = self.0.memory_settings().map_err(|error| read_failed(&error));
    #[cfg(feature = "test-agent")]
    sent::sending(&answer);
    answer
  }
}
