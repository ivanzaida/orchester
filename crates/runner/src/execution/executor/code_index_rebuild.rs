//! *Rebuild* and *Retry* of a code index in Project settings › Context ([`orchester_ipc::Command::RebuildCodeIndex`]):
//! the index is built again where the runner keeps the indexes its runs' agents read, beside the executor
//! (`docs/architecture/remote-runners.md`), never by the desktop.

use orchester_core::revision::Revision;
use orchester_index::IndexError;
use orchester_ipc::Rejection;

use crate::{execution::Scheduler, execution_root::ExecutionRoot};

impl Scheduler {
  /// Builds the index of `revision` again in the executor's indexes folder (its execution root's), off the runtime's
  /// workers; why it was not built otherwise.
  pub async fn rebuild_code_index(&self, revision: Revision) -> Result<(), Rejection> {
    let indexes = ExecutionRoot::local(&self.inner.project_root).code_indexes();
    let built = tokio::task::spawn_blocking(move || {
      indexes.rebuild(&revision).map(drop).map_err(|error| {
        tracing::warn!(%error, %revision, "the codebase index could not be rebuilt");
        rejection(error)
      })
    })
    .await;
    built.unwrap_or_else(|error| {
      Err(Rejection::Failed {
        detail: format!("building the index stopped: {error}"),
      })
    })
  }
}

/// What the desktop is told of an index that was not rebuilt.
fn rejection(error: IndexError) -> Rejection {
  match error {
    IndexError::InUse { .. } => Rejection::CodeIndexInUse,
    error => Rejection::Failed {
      detail: error.to_string(),
    },
  }
}
