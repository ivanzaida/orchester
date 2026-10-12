use super::{LocalIntegrationStore, LocalPath};
use crate::{blob::ContentHash, id::RunId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryLayer {
  Head,
  Staged,
  Working,
  TaskAncestor,
  TaskCurrent,
  TaskIncoming,
  WorkingBase,
  Result,
}

impl RecoveryLayer {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Head => "head",
      Self::Staged => "staged",
      Self::Working => "working",
      Self::TaskAncestor => "task_ancestor",
      Self::TaskCurrent => "task_current",
      Self::TaskIncoming => "task_incoming",
      Self::WorkingBase => "working_base",
      Self::Result => "result",
    }
  }
}

/// Original bytes are stored separately from journal metadata, never exposed through Debug or MCP.
pub trait LocalRecoveryStore: LocalIntegrationStore {
  fn retain_local_content(
    &mut self,
    run: RunId,
    path: &LocalPath,
    layer: RecoveryLayer,
    hash: &ContentHash,
    content: &[u8],
  ) -> Result<(), Self::Error>;

  fn local_content(&self, run: RunId, path: &LocalPath, layer: RecoveryLayer) -> Result<Option<Vec<u8>>, Self::Error>;

  fn remove_local_content(&mut self, run: RunId) -> Result<(), Self::Error>;
}
