//! What the Agents hub's tabs count (ORC-107): the project's profiles, instructions, skills and tools, and the memory
//! proposals waiting for a person, which Memory's tab shows in its warning tone.

use std::path::Path;

use orchester_core::{
  memory::{MemoryFilter, MemoryStatus, MemoryStore, ScopeFilter},
  resource::ResourceKind,
  store::ResourceStore,
};
use orchester_store::{ProjectDb, StoreError};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HubCounts {
  pub profiles: usize,
  pub instructions: usize,
  pub skills: usize,
  pub tools: usize,
  /// Memory records of the project and its tasks that wait for approval.
  pub waiting: usize,
}

impl HubCounts {
  pub fn load_in(repo_root: &Path) -> Result<Self, StoreError> {
    let db = ProjectDb::open(repo_root)?;
    let waiting = db.memories(&MemoryFilter {
      scope: ScopeFilter::All,
      status: Some(MemoryStatus::Proposed),
      pinned_only: false,
    })?;
    Ok(Self {
      profiles: db.resources(ResourceKind::AgentProfile)?.len(),
      instructions: db.resources(ResourceKind::Instruction)?.len(),
      skills: db.resources(ResourceKind::Skill)?.len(),
      tools: db.resources(ResourceKind::Tool)?.len(),
      waiting: waiting.len(),
    })
  }
}
