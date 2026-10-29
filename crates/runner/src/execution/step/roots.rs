//! The roots Autopilot answers a step's requests by (`orchester_core::permission::roots`,
//! `docs/design/approval-modes.md`): the worktree as its agent starts in it, the run's branch and Sources, the
//! repository's main checkout, and the folders the step's CLI keeps its own files of the run in.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{AgentConnection, AgentProvider, invocation::Platform},
  id::StepExecutionId,
  permission::roots::{AgentHomes, RunRoots},
};

use super::super::run::RunTask;

impl RunTask {
  /// The roots of `execution`, whose agent works in `worktree`; `None` when the worktree or the main checkout cannot be
  /// read, so that Autopilot places nothing inside. Without the step's connection, its CLI's own folders are left out.
  pub(super) fn run_roots(&self, execution: StepExecutionId, worktree: &Path) -> Option<RunRoots> {
    let roots = RunRoots::new(Platform::current(), worktree, self.run.branch().as_str())
      .and_then(|roots| roots.with_main_checkout(&self.root));
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
    let Some((_, connection)) = connection else {
      return Some(roots);
    };
    let provider = connection.provider();
    let temp = std::env::temp_dir();
    Some(match config_folder(&connection, provider) {
      Some(config) => roots.with_agent_temp(
        provider,
        AgentHomes {
          temp: &temp,
          config: &config,
        },
      ),
      None => roots,
    })
  }
}

/// The folder `provider`'s CLI keeps its configuration in: its account's, else the one Orchester's environment names
/// (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`), else its default in the home folder; `None` for a native agent.
fn config_folder(connection: &AgentConnection, provider: AgentProvider) -> Option<PathBuf> {
  if let Some((_, folder)) = connection.account().variable(provider) {
    return Some(folder.to_path_buf());
  }
  let (variable, default) = match provider {
    AgentProvider::ClaudeCodeCli => ("CLAUDE_CONFIG_DIR", ".claude"),
    AgentProvider::CodexCli => ("CODEX_HOME", ".codex"),
    AgentProvider::AnthropicApi | AgentProvider::OpenAiApi => return None,
  };
  std::env::var_os(variable)
    .map(PathBuf::from)
    .or_else(|| std::env::home_dir().map(|home| home.join(default)))
}
