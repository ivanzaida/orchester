//! Test suites (`docs/architecture/agent-testing.md`, "Test suite"): a program Orchester runs in a
//! run's worktree to test it, the report it reads back, the layer of the Test step it belongs to and
//! the services or stacks it needs. Its name and slug are the resource's.

mod command;
mod paths;
mod report;
mod stored;
mod variables;

use thiserror::Error;

pub use self::{
  command::{SuiteCommand, TestFilter},
  paths::{ArtifactGlob, SuitePath},
  report::{ReportFormat, ReportOutput, TestReport},
  variables::{SuiteVariable, SuiteVariables},
};
use crate::{
  resource::{CommandError, CommandTimeout, Slug, SlugError},
  secret::SecretNameError,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TestSuiteError {
  /// The program, an argument, the timeout or a variable breaks a rule of command tools.
  #[error(transparent)]
  Command(#[from] CommandError),
  #[error("the program cannot hold a placeholder; `{{{{filter}}}}` goes in an argument")]
  PlaceholderInProgram,
  #[error("`{0}` is not a placeholder of a test suite; the only one is `{{{{filter}}}}`")]
  UnknownPlaceholder(String),
  #[error("`{0}` opens a placeholder with `{{{{` but never closes it")]
  UnclosedPlaceholder(String),
  /// The filter fills one argument; a second would run the program with it twice.
  #[error("`{{{{filter}}}}` may stand in one argument only, once")]
  RepeatedFilter,
  #[error("`{0}` is not a report format: cargo_json, junit_xml, vitest_json, playwright_json or tap")]
  UnknownReportFormat(String),
  #[error("`{0}` is not a layer: lint, unit or e2e")]
  UnknownLayer(String),
  #[error("a {0} report is a file, so the suite needs its path")]
  ReportPathMissing(ReportFormat),
  #[error("a {0} report is printed, so it has no path")]
  ReportPrinted(ReportFormat),
  #[error("a path must not be blank")]
  BlankPath,
  #[error("`{0}` is not a path inside the worktree: it starts from a drive or a root")]
  AbsolutePath(String),
  #[error("`{0}` leaves the worktree")]
  PathLeavesWorktree(String),
  #[error(transparent)]
  SecretName(#[from] SecretNameError),
  #[error(transparent)]
  Need(#[from] SlugError),
  #[error("`{0}` is needed twice")]
  RepeatedNeed(String),
}

/// The layer of the Test step a suite runs in: lint, then unit, then e2e, stopping at the first that
/// fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TestLayer {
  Lint,
  Unit,
  E2e,
}

impl TestLayer {
  pub const ALL: [Self; 3] = [Self::Lint, Self::Unit, Self::E2e];

  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Lint => "lint",
      Self::Unit => "unit",
      Self::E2e => "e2e",
    }
  }

  pub fn parse(value: &str) -> Result<Self, TestSuiteError> {
    Self::ALL
      .into_iter()
      .find(|layer| layer.as_str() == value)
      .ok_or_else(|| TestSuiteError::UnknownLayer(value.to_owned()))
  }
}

/// The services or stacks, by slug, started before the suite and stopped after it, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SuiteNeeds(Vec<Slug>);

impl SuiteNeeds {
  pub fn new(needs: Vec<Slug>) -> Result<Self, TestSuiteError> {
    for (index, need) in needs.iter().enumerate() {
      if needs[..index].contains(need) {
        return Err(TestSuiteError::RepeatedNeed(need.as_str().to_owned()));
      }
    }
    Ok(Self(needs))
  }

  pub fn as_slice(&self) -> &[Slug] {
    &self.0
  }
}

/// A test suite's body. Its working folder is the worktree unless it names a folder inside.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestSuite {
  command: SuiteCommand,
  working_folder: Option<SuitePath>,
  variables: SuiteVariables,
  report: TestReport,
  layer: TestLayer,
  needs: SuiteNeeds,
  artifacts: Vec<ArtifactGlob>,
  timeout: CommandTimeout,
  always: bool,
}

impl TestSuite {
  /// `command` in the worktree, without variables, needs or artifacts, with the command tools'
  /// default timeout (10 minutes), run only when a run changed a path it owns.
  pub fn new(command: SuiteCommand, report: TestReport, layer: TestLayer) -> Self {
    Self {
      command,
      working_folder: None,
      variables: SuiteVariables::default(),
      report,
      layer,
      needs: SuiteNeeds::default(),
      artifacts: Vec::new(),
      timeout: CommandTimeout::DEFAULT,
      always: false,
    }
  }

  #[must_use]
  pub fn with_working_folder(mut self, folder: Option<SuitePath>) -> Self {
    self.working_folder = folder;
    self
  }

  #[must_use]
  pub fn with_variables(mut self, variables: SuiteVariables) -> Self {
    self.variables = variables;
    self
  }

  #[must_use]
  pub fn with_needs(mut self, needs: SuiteNeeds) -> Self {
    self.needs = needs;
    self
  }

  #[must_use]
  pub fn with_artifacts(mut self, artifacts: Vec<ArtifactGlob>) -> Self {
    self.artifacts = artifacts;
    self
  }

  #[must_use]
  pub const fn with_timeout(mut self, timeout: CommandTimeout) -> Self {
    self.timeout = timeout;
    self
  }

  /// Whether the Test step runs the suite even when the run changed no path it owns.
  #[must_use]
  pub const fn run_always(mut self, always: bool) -> Self {
    self.always = always;
    self
  }

  pub const fn command(&self) -> &SuiteCommand {
    &self.command
  }

  pub const fn working_folder(&self) -> Option<&SuitePath> {
    self.working_folder.as_ref()
  }

  pub const fn variables(&self) -> &SuiteVariables {
    &self.variables
  }

  pub const fn report(&self) -> &TestReport {
    &self.report
  }

  pub const fn layer(&self) -> TestLayer {
    self.layer
  }

  pub const fn needs(&self) -> &SuiteNeeds {
    &self.needs
  }

  pub fn artifacts(&self) -> &[ArtifactGlob] {
    &self.artifacts
  }

  pub const fn timeout(&self) -> CommandTimeout {
    self.timeout
  }

  pub const fn always(&self) -> bool {
    self.always
  }
}
