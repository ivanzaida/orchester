//! The programs the desktop runs for GitHub sync (`gh`), shared with screens through lurq
//! context. The application runs real processes; tests hand in a runner that answers with
//! captured output, so screens that talk to GitHub can be driven without touching the real `gh`.

use std::{sync::Arc, time::Duration};

use orchester_core::command::CommandRunner;
use orchester_exec::ProcessRunner;

/// A `gh` that does not answer within this time counts as not running, as for connection checks.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct Programs(Arc<dyn CommandRunner + Send + Sync>);

impl Programs {
  /// Real child processes.
  pub fn processes() -> Self {
    Self(Arc::new(ProcessRunner::new(COMMAND_TIMEOUT)))
  }

  /// Programs run by `runner`, e.g. captured answers in tests.
  pub fn with_runner(runner: Arc<dyn CommandRunner + Send + Sync>) -> Self {
    Self(runner)
  }

  pub fn runner(&self) -> &(dyn CommandRunner + Send + Sync) {
    &*self.0
  }
}

impl PartialEq for Programs {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl lurq::app::component::DevtoolsInspectable for Programs {}
