//! The files of a project's runs, read by the project's runner ([`Command::RunFiles`],
//! `docs/architecture/remote-runners.md`, rule 5): the code viewer never opens a run's worktree itself. A project
//! whose runner is not running gets one, started and attached as opening the project starts it.

use orchester_core::id::RunId;
use orchester_ipc::{
  Command, Reply,
  run_files::{RunFilesReply, RunFilesRequest},
};

use super::Runners;
use crate::{
  files::{FilesError, RunFiles},
  workspace::OpenProject,
};

/// The runs' files of one project, asked of its runner.
#[derive(Clone)]
pub struct ProjectRunFiles {
  runners: Runners,
  project: OpenProject,
}

/// What answers for the runner about a project's (its folder's) runs' files in tests that start no runner: the
/// runner's own reading, in-process (`orchester_runner::execution::answer_run_files`).
#[cfg(feature = "test-support")]
pub type RunFilesStandIn =
  std::sync::Arc<dyn Fn(&std::path::Path, RunId, RunFilesRequest) -> RunFilesReply + Send + Sync>;

/// Where [`Runners::serve_run_files_for_test`] keeps its stand-in; empty outside the `test-support` feature.
#[derive(Default)]
pub(super) struct StandIn(#[cfg(feature = "test-support")] parking_lot::Mutex<Option<RunFilesStandIn>>);

impl Runners {
  /// The files of `project`'s runs, which its runner reads.
  pub fn run_files(&self, project: &OpenProject) -> ProjectRunFiles {
    ProjectRunFiles {
      runners: self.clone(),
      project: project.clone(),
    }
  }

  /// From now on `stand_in` answers about runs' files instead of the projects' runners, for tests that start none.
  #[cfg(feature = "test-support")]
  #[doc(hidden)]
  pub fn serve_run_files_for_test(&self, stand_in: RunFilesStandIn) {
    *self.inner.run_files_stand_in.0.lock() = Some(stand_in);
  }
}

impl RunFiles for ProjectRunFiles {
  /// Blocks on the runners' runtime until the runner answers: never call it on an async worker.
  fn ask(&self, run: RunId, request: RunFilesRequest) -> Result<RunFilesReply, FilesError> {
    #[cfg(feature = "test-support")]
    if let Some(stand_in) = self.runners.inner.run_files_stand_in.0.lock().clone() {
      return Ok(stand_in(&self.project.repo_root, run, request));
    }
    let (runners, project) = (self.runners.clone(), self.project.clone());
    let asked = self.runners.inner.runtime.block_on(async move {
      runners
        .ask_project(&project, Command::RunFiles { run, request })
        .await
    });
    match asked {
      Ok(Reply::RunFiles(reply)) => Ok(reply),
      Ok(Reply::Rejected(rejection)) => Err(FilesError::Runner(format!("{rejection:?}"))),
      Ok(other) => {
        tracing::error!(?other, "unexpected reply to RunFiles");
        Err(FilesError::Runner("an unexpected reply".to_owned()))
      }
      Err(error) => Err(FilesError::Runner(error.to_string())),
    }
  }
}
