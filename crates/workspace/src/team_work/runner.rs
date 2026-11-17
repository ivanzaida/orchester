//! The runner a team start asks to start its run, once the run's inputs are kept, and that stops a run whose authority
//! the server took away: a real boundary, the project's runner process ([`ProjectRunner`]) in the app, the runner's own
//! scheduler where a test drives it directly.

use std::path::PathBuf;

use orchester_core::id::{OperationId, RunId};

use crate::runner::{RunRequestError, Runners};

/// Starts the run of the team inputs an accepted operation kept (protocol 42's `StartAcceptedRun`), and stops a run.
pub trait TeamRunner {
  /// Idempotent by the operation: it has one run, and asking again answers with it.
  fn start_accepted_run(&self, operation: OperationId) -> impl Future<Output = Result<RunId, RunRequestError>> + Send;

  /// Cancels run `run`, which then executes no more. A runner that is not attached is
  /// [`crate::runner::RequestError::NotConnected`]: it executes nothing now.
  fn cancel_run(&self, run: RunId) -> impl Future<Output = Result<(), RunRequestError>> + Send;
}

/// One project's runner process, started if it is not running.
pub struct ProjectRunner {
  runners: Runners,
  root: PathBuf,
  name: String,
}

impl ProjectRunner {
  /// The runner of the project at `root`, named `name`, of `runners`.
  pub const fn new(runners: Runners, root: PathBuf, name: String) -> Self {
    Self { runners, root, name }
  }
}

impl TeamRunner for ProjectRunner {
  fn start_accepted_run(&self, operation: OperationId) -> impl Future<Output = Result<RunId, RunRequestError>> + Send {
    self.runners.start_accepted_run(&self.root, &self.name, operation)
  }

  fn cancel_run(&self, run: RunId) -> impl Future<Output = Result<(), RunRequestError>> + Send {
    self.runners.cancel_run_at(&self.root, run)
  }
}
