//! The program a test suite starts and its arguments, with the `{{filter}}` placeholder: an argument
//! holding it is filled with the filter `run_tests` was given, and left out when it was given none.

use super::TestSuiteError;
use crate::resource::{CommandArg, CommandError, CommandProgram};

const FILTER: &str = "{{filter}}";

/// What `run_tests(suite, filter)` narrows a suite to, e.g. `skills_migration`. It is passed to the
/// program as an argument, so it obeys the rules of one ([`CommandArg`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestFilter(String);

impl TestFilter {
  pub fn new(value: &str) -> Result<Self, CommandError> {
    CommandArg::new(value).map(|arg| Self(arg.as_str().to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// A program and its arguments, without a shell. `{{filter}}` is the only placeholder; it may stand
/// in one argument, once, and never in the program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiteCommand {
  program: CommandProgram,
  args: Vec<CommandArg>,
}

impl SuiteCommand {
  pub fn new(program: CommandProgram, args: Vec<CommandArg>) -> Result<Self, TestSuiteError> {
    if program.as_str().contains("{{") {
      return Err(TestSuiteError::PlaceholderInProgram);
    }
    let mut filters = 0;
    for arg in &args {
      filters += filters_in(arg.as_str())?;
    }
    if filters > 1 {
      return Err(TestSuiteError::RepeatedFilter);
    }
    Ok(Self { program, args })
  }

  pub const fn program(&self) -> &CommandProgram {
    &self.program
  }

  pub fn args(&self) -> &[CommandArg] {
    &self.args
  }

  /// Whether one of the arguments takes a filter.
  pub fn accepts_filter(&self) -> bool {
    self.args.iter().any(|arg| arg.as_str().contains(FILTER))
  }

  /// The arguments the program gets: `{{filter}}` replaced by `filter`, and the argument that holds
  /// it left out without one.
  pub fn arguments(&self, filter: Option<&TestFilter>) -> Vec<String> {
    self
      .args
      .iter()
      .filter_map(|arg| {
        let arg = arg.as_str();
        match filter {
          _ if !arg.contains(FILTER) => Some(arg.to_owned()),
          Some(filter) => Some(arg.replace(FILTER, filter.as_str())),
          None => None,
        }
      })
      .collect()
  }
}

/// How many times `{{filter}}` stands in `arg`; an error for any other `{{…}}`, or a `{{` that is
/// never closed.
fn filters_in(arg: &str) -> Result<usize, TestSuiteError> {
  let mut count = 0;
  let mut rest = arg;
  while let Some(start) = rest.find("{{") {
    let after = &rest[start + 2..];
    let end = after
      .find("}}")
      .ok_or_else(|| TestSuiteError::UnclosedPlaceholder(arg.to_owned()))?;
    let placeholder = &rest[start..start + 2 + end + 2];
    if placeholder != FILTER {
      return Err(TestSuiteError::UnknownPlaceholder(placeholder.to_owned()));
    }
    count += 1;
    rest = &after[end + 2..];
  }
  Ok(count)
}
