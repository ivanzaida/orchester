//! The codebase index of a run's base revision (`orchester_index`): built, or reused, each time the
//! run's task starts executing it, before any step starts, and kept while the run's worktree
//! exists. An index that cannot be built does not stop the run: each agent that starts without it
//! gets a [`RunnerProblem::CodebaseIndexUnavailable`] in its activity saying why.

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::RunId,
  revision::Revision,
};
use orchester_index::{CodebaseIndexes, Outcome};

use super::run::RunTask;

/// Builds or reuses the index of `revision` for `run`, off the runtime's threads; why it is
/// unavailable, if it is.
pub(super) async fn ensure(indexes: CodebaseIndexes, revision: Revision, run: RunId) -> Option<String> {
  let ensured = tokio::task::spawn_blocking(move || indexes.ensure(&revision, run)).await;
  match ensured {
    Ok(Ok(ensured)) => {
      match ensured.outcome {
        Outcome::Reused => tracing::info!(%run, path = %ensured.path.display(), "reusing the codebase index"),
        Outcome::Built(report) => tracing::info!(
          %run,
          path = %ensured.path.display(),
          mode = ?report.mode,
          files = report.files,
          read = report.read,
          millis = report.duration.as_millis(),
          "built the codebase index"
        ),
      }
      None
    }
    Ok(Err(error)) => {
      tracing::warn!(%run, %error, "the codebase index is unavailable");
      Some(error.to_string())
    }
    Err(error) => {
      tracing::error!(%run, %error, "building the codebase index stopped");
      Some(format!("building the index stopped: {error}"))
    }
  }
}

impl RunTask {
  /// The event an agent starting now gets when the run has no codebase index.
  pub(super) fn index_unavailable(&self) -> Option<AgentEvent> {
    self.index_problem.as_ref().map(|detail| AgentEvent::RunnerProblem {
      problem: RunnerProblem::CodebaseIndexUnavailable { detail: detail.clone() },
    })
  }
}
