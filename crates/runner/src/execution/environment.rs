//! A configured Cargo directory is a build root. Each worktree gets its own artifacts;
//! agent and command invocations in that worktree reuse the same directory after a restart.

use std::{collections::BTreeMap, path::Path};

use orchester_core::{connection::stream::RunnerProblem, resource::CommandEnvironment, store::SettingsStore};

use super::run::RunTask;

impl RunTask {
  pub(super) fn environment(&self, worktree: &Path) -> Result<CommandEnvironment, RunnerProblem> {
    let environment = self.db.project_environment().map_err(problem)?;
    for_worktree(environment, worktree)
  }
}

pub(super) fn for_worktree(
  environment: CommandEnvironment,
  worktree: &Path,
) -> Result<CommandEnvironment, RunnerProblem> {
  let Some(configured) = environment.get("CARGO_TARGET_DIR") else {
    return Ok(environment);
  };
  let root = Path::new(configured);
  // Relative targets already belong to their process's worktree.
  if !root.is_absolute() {
    return Ok(environment);
  }
  let directory = worktree
    .file_name()
    .ok_or_else(|| problem("the worktree has no directory name"))?;
  let target = root.join("worktrees").join(directory);
  let value = target
    .to_str()
    .ok_or_else(|| problem("the Cargo target path is not Unicode"))?;
  let variables: BTreeMap<_, _> = environment
    .iter()
    .map(|(name, original)| {
      let value = if name.as_str().eq_ignore_ascii_case("CARGO_TARGET_DIR") {
        value
      } else {
        original
      };
      (name.clone(), value.to_owned())
    })
    .collect();
  CommandEnvironment::new(variables).map_err(problem)
}

fn problem(error: impl std::fmt::Display) -> RunnerProblem {
  RunnerProblem::CouldNotStart {
    detail: error.to_string(),
  }
}
