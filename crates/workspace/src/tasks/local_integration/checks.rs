use orchester_core::{
  connection::invocation::Platform,
  integration::local::{CheckedResult, LocalError, LocalPhase},
  resource::{Tool, ToolAction},
  run::StepExecutor,
  store::{ResourceStore, SettingsStore},
};

use super::{IntegrationError, LocalIntegration};

impl LocalIntegration {
  pub(super) fn run_check(&mut self) -> Result<(), IntegrationError> {
    let before = CheckedResult {
      base: self.main.verified_snapshot()?,
      result: self.task_git()?.snapshot()?,
    };
    if !before.base.clean() || !before.result.clean() {
      return Err(LocalError::Unresolved.into());
    }
    let actions = self.check_actions()?;
    let environment = self.db.project_environment()?;
    #[cfg(windows)]
    let platform = Platform::Windows;
    #[cfg(not(windows))]
    let platform = Platform::MacOs;
    for action in actions {
      let path = action.search_path(&environment, std::env::var_os("PATH"));
      let program = action
        .program_candidates(self.task_git()?.root(), path.as_deref(), platform)
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or(IntegrationError::NoCheck)?;
      action
        .program()
        .check_place(&program, self.main.root())
        .map_err(|_| LocalError::Unsupported)?;
      let command = action.process(
        &program,
        self.task_git()?.root(),
        std::env::vars_os(),
        &environment,
        platform,
      );
      // Check output can include arbitrary project secrets; only its actual exit outcome is retained here.
      let code = self.task_git()?.check_command(&command, action.timeout().duration())?;
      if !code.is_some_and(|code| action.exit_codes().contains(i64::from(code))) {
        self.operation.paused();
        self.save()?;
        return Err(IntegrationError::CheckFailed);
      }
    }
    let after = CheckedResult {
      base: self.main.verified_snapshot()?,
      result: self.task_git()?.snapshot()?,
    };
    let mut checked_input = before;
    // Check is allowed to produce ignored build output in the task worktree.
    checked_input.result.ignored = after.result.ignored.clone();
    if checked_input != after {
      return Err(LocalError::Stale.into());
    }
    self.operation.checked_result(after)?;
    self.save()
  }

  fn check_actions(&self) -> Result<Vec<orchester_core::resource::CommandAction>, IntegrationError> {
    let snapshot = self.run.snapshot();
    let mut actions = Vec::new();
    for step in snapshot.steps().iter().filter(|step| step.key().as_str() == "check") {
      let StepExecutor::Tool { tool } = step.executor() else {
        return Err(IntegrationError::NoCheck);
      };
      let version = self
        .db
        .version::<Tool>(tool.resource, tool.number)?
        .ok_or(IntegrationError::NoCheck)?;
      if version.id() != tool.version {
        return Err(LocalError::Stale.into());
      }
      let ToolAction::Command(action) = version.body().action() else {
        return Err(IntegrationError::NoCheck);
      };
      // An access policy needing runtime approval must remain with the runner's protected tool path.
      if version.body().access().is_some() {
        return Err(IntegrationError::NoCheck);
      }
      actions.push(action.clone());
    }
    if actions.is_empty() {
      return Err(IntegrationError::NoCheck);
    }
    Ok(actions)
  }

  pub fn recheck(&mut self) -> Result<(), IntegrationError> {
    if !matches!(
      self.operation.phase(),
      LocalPhase::Checking | LocalPhase::Prepared | LocalPhase::Paused
    ) {
      return Err(LocalError::WrongPhase.into());
    }
    if self.operation.phase() != LocalPhase::Checking {
      self.operation.prepared(self.main.verified_snapshot()?)?;
      self.operation.begin_check()?;
      self.save()?;
    }
    self.run_check()
  }
}
