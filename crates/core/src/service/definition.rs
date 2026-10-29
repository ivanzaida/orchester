use std::collections::BTreeSet;

use super::{OwnedPath, Readiness, RelativeFolder, RunFolder, Seed, ServiceError, ServiceName, ServiceVariables};
use crate::resource::{CommandArg, CommandProgram};

/// The data a run gets of its own, deleted when the run ends.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RunData {
  None,
  /// A fresh database container per run.
  DatabaseContainer,
  /// Files in a folder of the run's own.
  RunFolder(RunFolder),
}

/// How agents see and use the service's UI through the `ui_*` tools. Browser automation comes later through the
/// in-app Browser (`docs/architecture/agent-testing.md`, decision 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UiDriver {
  /// No UI: logs and HTTP only.
  None,
  /// Windows UI Automation, macOS Accessibility.
  Native,
  /// An app with its own MCP server, such as lurq apps and Orchester.
  Mcp,
}

/// Where `ui_open` starts, e.g. `{{url}}/teams/{{seed.team_id}}` or an app's route; its placeholders are kept as
/// written.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct UiStart(String);

impl UiStart {
  pub const MAX_CHARS: usize = 4096;

  pub fn new(value: &str) -> Result<Self, ServiceError> {
    let valid = !value.is_empty()
      && value.chars().count() <= Self::MAX_CHARS
      && !value.chars().any(|c| c.is_whitespace() || c.is_control());
    if !valid {
      return Err(ServiceError::UiStart);
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// A program Orchester starts for a run, without a shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
  name: ServiceName,
  program: CommandProgram,
  arguments: Vec<CommandArg>,
  working_folder: RelativeFolder,
  variables: ServiceVariables,
  needs: Vec<ServiceName>,
  readiness: Readiness,
  seed: Seed,
  run_data: RunData,
  ui_driver: UiDriver,
  ui_start: Option<UiStart>,
  owned_paths: Vec<OwnedPath>,
}

impl Service {
  /// `program` started in the worktree with no arguments, variables or needs, no seed, no data of its own, no UI
  /// driver and no owned paths.
  pub fn new(name: ServiceName, program: CommandProgram, readiness: Readiness) -> Self {
    Self {
      name,
      program,
      arguments: Vec::new(),
      working_folder: RelativeFolder::root(),
      variables: ServiceVariables::default(),
      needs: Vec::new(),
      readiness,
      seed: Seed::None,
      run_data: RunData::None,
      ui_driver: UiDriver::None,
      ui_start: None,
      owned_paths: Vec::new(),
    }
  }

  #[must_use]
  pub fn with_arguments(mut self, arguments: Vec<CommandArg>) -> Self {
    self.arguments = arguments;
    self
  }

  #[must_use]
  pub fn with_working_folder(mut self, folder: RelativeFolder) -> Self {
    self.working_folder = folder;
    self
  }

  #[must_use]
  pub fn with_variables(mut self, variables: ServiceVariables) -> Self {
    self.variables = variables;
    self
  }

  /// The services started and ready before this one, each once and none of them this service.
  pub fn with_needs(mut self, needs: Vec<ServiceName>) -> Result<Self, ServiceError> {
    if needs.contains(&self.name) {
      return Err(ServiceError::NeedsItself(self.name));
    }
    let mut seen = BTreeSet::new();
    if let Some(need) = needs.iter().find(|need| !seen.insert(*need)) {
      return Err(ServiceError::DuplicateNeed(need.clone()));
    }
    self.needs = needs;
    Ok(self)
  }

  #[must_use]
  pub fn with_seed(mut self, seed: Seed) -> Self {
    self.seed = seed;
    self
  }

  #[must_use]
  pub fn with_run_data(mut self, run_data: RunData) -> Self {
    self.run_data = run_data;
    self
  }

  #[must_use]
  pub const fn with_ui_driver(mut self, driver: UiDriver) -> Self {
    self.ui_driver = driver;
    self
  }

  #[must_use]
  pub fn with_ui_start(mut self, start: UiStart) -> Self {
    self.ui_start = Some(start);
    self
  }

  /// The repository path patterns this service owns, each once.
  pub fn with_owned_paths(mut self, paths: Vec<OwnedPath>) -> Result<Self, ServiceError> {
    let mut seen = BTreeSet::new();
    if let Some(path) = paths.iter().find(|path| !seen.insert(path.as_str())) {
      return Err(ServiceError::DuplicateOwnedPath(path.as_str().to_owned()));
    }
    self.owned_paths = paths;
    Ok(self)
  }

  pub const fn name(&self) -> &ServiceName {
    &self.name
  }

  pub const fn program(&self) -> &CommandProgram {
    &self.program
  }

  pub fn arguments(&self) -> &[CommandArg] {
    &self.arguments
  }

  pub const fn working_folder(&self) -> &RelativeFolder {
    &self.working_folder
  }

  pub const fn variables(&self) -> &ServiceVariables {
    &self.variables
  }

  pub fn needs(&self) -> &[ServiceName] {
    &self.needs
  }

  pub const fn readiness(&self) -> &Readiness {
    &self.readiness
  }

  pub const fn seed(&self) -> &Seed {
    &self.seed
  }

  pub const fn run_data(&self) -> &RunData {
    &self.run_data
  }

  pub const fn ui_driver(&self) -> UiDriver {
    self.ui_driver
  }

  /// Where `ui_open` starts; `None` leaves it to the driver.
  pub const fn ui_start(&self) -> Option<&UiStart> {
    self.ui_start.as_ref()
  }

  pub fn owned_paths(&self) -> &[OwnedPath] {
    &self.owned_paths
  }
}
