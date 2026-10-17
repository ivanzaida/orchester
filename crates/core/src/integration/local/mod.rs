//! Durable local checkout integration. Editing a result and approving its checked identity are separate decisions.

mod draft;
mod hook;
mod path;
mod process;
mod rebase;
mod record;
mod recovery;
mod resolution;

pub use draft::{ConflictKind, DraftChoice, DraftIdentity, DraftRecord, EditorCandidate};
pub use hook::HookChange;
pub use path::LocalPath;
pub use process::{ExecutorIdentity, LocalProcess, LocalProcessStore, ProcessIdentity, ProcessJournal};
pub use rebase::{RebaseEvidence, RebaseStore, RebasedCheckpoint, RunRebase};
pub use record::{CheckedResult, LocalOperation, LocalPhase, Preservation};
pub use recovery::{LocalRecoveryStore, RecoveryLayer};
pub use resolution::{PendingResolution, ResolutionWrite};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{blob::ContentHash, id::RunId};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LocalError {
  #[error("the checkout changed since it was reviewed")]
  Stale,
  #[error("the initiating action authority changed or was revoked")]
  AuthorityChanged,
  #[error("another Git operation owns this checkout")]
  ForeignOperation,
  #[error("this operation cannot run in the current phase")]
  WrongPhase,
  #[error("the result has not passed a fresh Check")]
  NotChecked,
  #[error("the exact checked result needs a person's approval")]
  ApprovalRequired,
  #[error("the preservation has not been verified")]
  NotVerified,
  #[error("the operation outcome needs reconciliation")]
  Reconcile,
  #[error("this path or file format is unsupported")]
  Unsupported,
  #[error("conditional file removal is unsupported on this platform; originals remain retained")]
  ConditionalRemovalUnsupported,
  #[error("owned process containment is unsupported on this platform")]
  ProcessContainmentUnsupported,
  #[error("owned recovery storage is unsupported on this platform")]
  StoreOwnershipUnsupported,
  #[error("owned descendants or pipes have not been proved stopped; recovery remains retained")]
  ProcessUncertain,
  #[error("the destination already exists")]
  Collision,
  #[error("unresolved files or conflict markers remain")]
  Unresolved,
  #[error("a commit message or preservation name is required")]
  Blank,
  #[error("a commit hook changed the reviewed commit; nothing was committed")]
  HookChanged,
}

/// Read-only Git evidence, not a historical Orchester merge record or approval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainedResult {
  pub kind: ResultEvidenceKind,
  pub source: crate::revision::Revision,
  pub target: crate::revision::Revision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultEvidenceKind {
  CompleteAncestry,
  PartialAncestry,
  EquivalentPatches,
  EquivalentTree,
  UnknownSource,
}

/// A regular file's Git representation; absent entries represent deletion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
  pub mode: u32,
  pub object: String,
}

/// Fingerprints only: file bytes never enter an operation's Debug or journal metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalFile {
  pub path: LocalPath,
  pub head: Option<IndexEntry>,
  pub staged: Option<IndexEntry>,
  pub working: Option<ContentHash>,
  pub executable: bool,
  #[serde(default)]
  pub unstaged: bool,
}

impl LocalFile {
  /// The path changed only in the working copy: HEAD and the index agree, so a commit of the index has nothing of
  /// it to take.
  pub fn only_unstaged(&self) -> bool {
    self.head.is_some() && self.head == self.staged
  }
}

/// Every relevant identity is compared before a mutation, including Git operation markers.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkout {
  pub root: String,
  pub git_dir: String,
  pub branch: String,
  pub head: String,
  pub index: ContentHash,
  pub files: Vec<LocalFile>,
  pub ignored: ContentHash,
  pub operation: Option<String>,
}

impl Checkout {
  pub fn ensure_current(&self, current: &Self) -> Result<(), LocalError> {
    if current.operation.is_some() {
      return Err(LocalError::ForeignOperation);
    }
    if self != current {
      return Err(LocalError::Stale);
    }
    Ok(())
  }

  pub fn clean(&self) -> bool {
    self.files.is_empty() && self.operation.is_none()
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Placement {
  Stage,
  LeaveUnstaged,
}

/// Restoration choices retain the originals even when a person chooses different result bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoredFile {
  pub path: LocalPath,
  #[serde(default)]
  pub before: Option<ContentHash>,
  pub executable: bool,
  pub working: Option<ContentHash>,
  pub staged: Option<IndexEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalConflict {
  pub path: LocalPath,
  pub ancestor: Option<IndexEntry>,
  pub current: Option<IndexEntry>,
  pub incoming: Option<IndexEntry>,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Resolution {
  Text(String),
  Current,
  Incoming,
  Delete,
  KeepBoth(LocalPath),
  AtPath { incoming: bool, destination: LocalPath },
}

/// Storage compares the saved version before replacing a journal; a stale writer changes nothing.
pub trait LocalIntegrationStore {
  type Error: std::error::Error + Send + Sync + 'static;

  fn local_operation(&self, run: RunId) -> Result<Option<LocalOperation>, Self::Error>;
  fn local_operations(&self) -> Result<Vec<LocalOperation>, Self::Error>;
  fn save_local_operation(&mut self, operation: &LocalOperation, expected: Option<u64>) -> Result<bool, Self::Error>;
}

impl std::fmt::Debug for Checkout {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Checkout")
      .field("head", &self.head)
      .field("index", &self.index)
      .field("files", &self.files.len())
      .field("metadata", &"<private>")
      .finish()
  }
}

impl std::fmt::Debug for Resolution {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(match self {
      Self::Text(_) => "Resolution::Text(<private>)",
      Self::Current => "Resolution::Current",
      Self::Incoming => "Resolution::Incoming",
      Self::Delete => "Resolution::Delete",
      Self::KeepBoth(_) => "Resolution::KeepBoth(<private>)",
      Self::AtPath { .. } => "Resolution::AtPath(<private>)",
    })
  }
}
