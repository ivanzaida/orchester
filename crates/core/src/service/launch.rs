//! How a service's program starts for a run (`docs/architecture/agent-testing.md`, "Service"): without a shell, in
//! its working folder inside the run's worktree, with its placeholders filled ([`RunValues`]), the cleaned environment
//! every program Orchester starts gets, the project's variables and the service's own on top. A variable that names a
//! secret stays a name here: the runner reads its value from the keychain and sets it last.

use std::{
  ffi::{OsStr, OsString},
  path::{Path, PathBuf},
};

use super::{PlaceholderError, RunValues, Seed, Service, ServiceValue};
use crate::{
  command::Command,
  connection::invocation::{Platform, agent_environment},
  resource::{CommandEnvironment, NO_CURRENT_DIRECTORY_IN_EXE_PATH, VariableName},
  secret::SecretName,
};

/// A service's process, ready to start once its secrets are read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceProcess {
  /// The program with its filled arguments, working folder and plain variables.
  pub command: Command,
  /// The variables whose value is the secret of that name, in the service's order.
  pub secrets: Vec<(VariableName, SecretName)>,
}

impl Service {
  /// The folder the program starts in: its working folder inside `worktree`.
  pub fn folder_in(&self, worktree: &Path) -> PathBuf {
    if self.working_folder().is_root() {
      worktree.to_path_buf()
    } else {
      worktree.join(self.working_folder().as_str())
    }
  }

  /// The `PATH` the program is looked up in and starts with: the service's own plain `PATH` when it sets one, else
  /// the one the project adds (`project`), else `inherited`.
  pub fn search_path(&self, project: &CommandEnvironment, inherited: Option<OsString>) -> Option<OsString> {
    let own = self.variables().iter().find_map(|(name, value)| match value {
      ServiceValue::Plain(text) if name.as_str().eq_ignore_ascii_case("PATH") => Some(text.as_str()),
      ServiceValue::Plain(_) | ServiceValue::Secret(_) => None,
    });
    own.or_else(|| project.get("PATH")).map(OsString::from).or(inherited)
  }

  /// Files the program may be on `platform` ([`crate::resource::CommandProgram::candidates`]), looked up in `path`.
  pub fn program_candidates(&self, worktree: &Path, path: Option<&OsStr>, platform: Platform) -> Vec<PathBuf> {
    self.program().candidates(worktree, path, platform)
  }

  /// The process that runs `program` (a file [`Self::program_candidates`] found) with the arguments `values` fill,
  /// in [`Self::folder_in`] `worktree`, with the allowed part of `environment`, then the variables the project adds
  /// (`project`), then the service's plain ones filled by `values`; each replaces one of the same name. On Windows the
  /// process is also told not to start programs from its current directory by name, as a command tool is
  /// ([`NO_CURRENT_DIRECTORY_IN_EXE_PATH`]).
  pub fn process(
    &self,
    (program, worktree): (&Path, &Path),
    (environment, project): (impl IntoIterator<Item = (OsString, OsString)>, &CommandEnvironment),
    platform: Platform,
    values: &RunValues,
  ) -> Result<ServiceProcess, PlaceholderError> {
    let arguments = values.arguments(self)?;
    self.process_of((program, arguments, worktree), (environment, project), platform, values)
  }

  /// The process of the seed command: `program` (a file its [`crate::resource::CommandProgram::candidates`] found)
  /// with the seed's arguments `values` fill, in the folder and with the environment [`Self::process`] gives the
  /// service's program, its secret variables too. `None` when the seed is not a command.
  pub fn seed_process(
    &self,
    (program, worktree): (&Path, &Path),
    (environment, project): (impl IntoIterator<Item = (OsString, OsString)>, &CommandEnvironment),
    platform: Platform,
    values: &RunValues,
  ) -> Result<Option<ServiceProcess>, PlaceholderError> {
    if !matches!(self.seed(), Seed::Command { .. }) {
      return Ok(None);
    }
    let arguments = values.seed_arguments(self)?;
    self
      .process_of((program, arguments, worktree), (environment, project), platform, values)
      .map(Some)
  }

  fn process_of(
    &self,
    (program, arguments, worktree): (&Path, Vec<String>, &Path),
    (environment, project): (impl IntoIterator<Item = (OsString, OsString)>, &CommandEnvironment),
    platform: Platform,
    values: &RunValues,
  ) -> Result<ServiceProcess, PlaceholderError> {
    let mut variables = agent_environment(environment);
    project.add_to(&mut variables);
    let mut secrets = Vec::new();
    for (name, value) in values.variables(self)? {
      variables.retain(|(existing, _)| !existing.eq_ignore_ascii_case(name.as_str()));
      match value {
        ServiceValue::Plain(text) => variables.push((name.as_str().to_owned(), text)),
        ServiceValue::Secret(secret) => secrets.push((name, secret)),
      }
    }
    if platform == Platform::Windows {
      let (name, value) = NO_CURRENT_DIRECTORY_IN_EXE_PATH;
      variables.retain(|(existing, _)| !existing.eq_ignore_ascii_case(name));
      variables.push((name.to_owned(), value.to_owned()));
    }
    let command = Command::new(program, arguments)
      .with_current_dir(&self.folder_in(worktree))
      .with_environment(variables);
    Ok(ServiceProcess { command, secrets })
  }
}
