//! The roots Autopilot answers a step's requests by (`orchester_core::permission::roots`,
//! `docs/design/approval-modes.md`): the worktree as its agent starts in it, the run's branch and Sources, the
//! repository's main checkout, the run's own storage a read of a design may export to, and the folders the step's CLI
//! keeps its own files of the run in.

use std::path::Path;

use orchester_core::{
  connection::invocation::Platform,
  id::StepExecutionId,
  permission::roots::{AgentHomes, RunRoots},
  workspaces::artifacts_folder,
};

use super::super::run::RunTask;

impl RunTask {
  /// The roots of `execution`, whose agent works in `worktree`; `None` when the worktree, the main checkout or the
  /// run's storage cannot be read, so that Autopilot places nothing inside. Without the step's connection, its CLI's
  /// own folders are left out; they are where the computer the run executes on says the CLI keeps its configuration
  /// (its account's folder, else the CLI's home), none for a native agent.
  pub(super) fn run_roots(&self, execution: StepExecutionId, worktree: &Path) -> Option<RunRoots> {
    let roots = RunRoots::new(Platform::current(), worktree, self.run.branch().as_str())
      .and_then(|roots| roots.with_main_checkout(&self.root))
      .and_then(|roots| roots.with_run_storage(&artifacts_folder(&self.root, self.run.id())));
    let roots = match roots {
      Ok(roots) => roots.with_sources(self.run.snapshot().sources()),
      Err(error) => {
        tracing::warn!(%error, "the run's roots cannot be read; Autopilot places none of its requests inside");
        return None;
      }
    };
    let connection = self
      .agent_step(execution)
      .ok()
      .and_then(|(step, profile, _)| self.profile_and_connection(profile, &step.key).ok());
    let Some((_, connection, here)) = connection else {
      return Some(roots);
    };
    let provider = connection.provider();
    let temp = std::env::temp_dir();
    Some(match here.config_folder() {
      Some(config) => roots.with_agent_temp(
        provider,
        AgentHomes {
          temp: &temp,
          config,
          user: orchester_exec::user::user_id(),
        },
      ),
      None => roots,
    })
  }
}
