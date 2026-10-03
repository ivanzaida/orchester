//! What a run's timeline is read from besides the task: its transcript and its tool requests,
//! pending and decided. Blocks on SQLite, so the UI calls it off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::RunId,
  run::{DecidedToolApproval, PendingToolApproval},
  store::ActivityStore,
};
use orchester_store::{ProjectDb, StoreError};

use crate::tasks::transcript::{Entry, entries};

/// A run's transcript and tool requests, as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loaded {
  pub entries: Arc<Vec<Entry>>,
  pub pending: Arc<Vec<PendingToolApproval>>,
  pub decided: Arc<Vec<DecidedToolApproval>>,
}

impl lurq::app::component::DevtoolsInspectable for Loaded {}

/// Reads `run`'s transcript and tool requests.
pub fn load_in(repo_root: &Path, run: RunId) -> Result<Loaded, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  Ok(Loaded {
    entries: Arc::new(entries(&db.activity(run, 0)?)),
    pending: Arc::new(db.pending_tool_approvals(run)?),
    decided: Arc::new(db.decided_tool_approvals(run)?),
  })
}
