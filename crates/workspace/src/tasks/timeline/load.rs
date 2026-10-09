//! What a run's timeline is read from besides the task: its transcript and its tool requests,
//! pending and decided, read apart. The requests are read again after each read of the transcript, so
//! they are never older than it, and as soon as the project's waiting requests change, so a decision
//! reaches the chat as it reaches the dialog and the status bar, while the transcript, much longer,
//! follows the run's output. Blocks on SQLite, so the UI calls it off the UI thread.

use std::{path::Path, sync::Arc};

use orchester_core::{
  id::RunId,
  question::AskedQuestions,
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
  pub questions: Arc<Vec<AskedQuestions>>,
}

impl lurq::app::component::DevtoolsInspectable for Loaded {}

/// A run's tool requests and its agents' questions with suggested answers, as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Requests {
  pub pending: Arc<Vec<PendingToolApproval>>,
  pub decided: Arc<Vec<DecidedToolApproval>>,
  pub questions: Arc<Vec<AskedQuestions>>,
}

impl lurq::app::component::DevtoolsInspectable for Requests {}

impl Loaded {
  /// The timeline's sources: `entries`, the transcript, with `requests`.
  pub fn of(entries: Arc<Vec<Entry>>, requests: Requests) -> Self {
    Self {
      entries,
      pending: requests.pending,
      decided: requests.decided,
      questions: requests.questions,
    }
  }
}

/// Reads `run`'s transcript.
pub fn transcript_in(repo_root: &Path, run: RunId) -> Result<Arc<Vec<Entry>>, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  Ok(Arc::new(entries(&db.activity(run, 0)?)))
}

/// Reads `run`'s tool requests.
pub fn requests_in(repo_root: &Path, run: RunId) -> Result<Requests, StoreError> {
  let db = ProjectDb::open(repo_root)?;
  Ok(Requests {
    pending: Arc::new(db.pending_tool_approvals(run)?),
    decided: Arc::new(db.decided_tool_approvals(run)?),
    questions: Arc::new(db.questions(run)?),
  })
}
