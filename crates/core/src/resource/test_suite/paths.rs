//! Paths a test suite names inside the run's worktree: its working folder, its report file and its
//! artifacts. Each is relative and stays inside; both separators count on every platform and are
//! kept as `/`, so a suite means the same on Windows and macOS.

use std::fmt;

use super::TestSuiteError;
use crate::resource::CommandArg;

/// A path relative to the worktree, or to the suite's working folder, e.g. `target/junit.xml`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SuitePath(String);

impl SuitePath {
  pub fn new(value: &str) -> Result<Self, TestSuiteError> {
    relative(value).map(Self)
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for SuitePath {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

/// Files a suite leaves for a person or an agent to look at, relative to its working folder:
/// `*` and `?` within a name, `**` across folders (`test-results/**/*.png`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactGlob(String);

impl ArtifactGlob {
  pub fn new(value: &str) -> Result<Self, TestSuiteError> {
    relative(value).map(Self)
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// `value` trimmed, with `/` between its parts: refused when blank, absolute, from a drive or a
/// root, climbing out with `..`, or not passable as an argument ([`CommandArg`]).
fn relative(value: &str) -> Result<String, TestSuiteError> {
  let value = value.trim();
  if value.is_empty() {
    return Err(TestSuiteError::BlankPath);
  }
  CommandArg::new(value)?;
  let mut chars = value.chars();
  let drive = chars.next().is_some_and(|letter| letter.is_ascii_alphabetic()) && chars.next() == Some(':');
  if drive || value.starts_with(['/', '\\']) {
    return Err(TestSuiteError::AbsolutePath(value.to_owned()));
  }
  let parts: Vec<&str> = value.split(['/', '\\']).collect();
  if parts.contains(&"..") {
    return Err(TestSuiteError::PathLeavesWorktree(value.to_owned()));
  }
  Ok(parts.join("/"))
}
