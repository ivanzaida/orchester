//! One `run_tests` call as the run keeps it (`docs/architecture/agent-testing.md`, "Agent tools"): which suite ran
//! with which filter, how its program ended, its report read into the shared shape or why it could not be, and the
//! artifacts copied into the run's storage. It is stored as JSON in the run's storage, so `test_output` and later
//! steps read it back.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use super::{EXCERPT_LINE_CHARS, TestCase, TestReport, excerpt};

/// How many of the program's last lines an unreadable report or a stop at the timeout shows.
pub const LAST_LINES: usize = 20;

/// Which `run_tests` call of a run a result is, `t1`, `t2`, … in the order they started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestRunId(u32);

impl TestRunId {
  pub const FIRST: Self = Self(1);

  pub const fn next(self) -> Self {
    Self(self.0.saturating_add(1))
  }
}

impl fmt::Display for TestRunId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "t{}", self.0)
  }
}

impl FromStr for TestRunId {
  type Err = String;

  fn from_str(value: &str) -> Result<Self, String> {
    value
      .strip_prefix('t')
      .filter(|digits| !digits.is_empty() && digits.len() <= 9 && digits.bytes().all(|byte| byte.is_ascii_digit()))
      .and_then(|digits| digits.parse().ok())
      .filter(|number| *number > 0)
      .map(Self)
      .ok_or_else(|| format!("`{value}` is not a test run: run_tests names them t1, t2, …"))
  }
}

impl Serialize for TestRunId {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(self)
  }
}

impl<'de> Deserialize<'de> for TestRunId {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    String::deserialize(deserializer)?
      .parse()
      .map_err(serde::de::Error::custom)
  }
}

/// How a suite's run ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SuiteRunEnd {
  /// The report was read.
  Read { report: TestReport },
  /// The report is missing, empty, cut off or not of its format: never a pass.
  Unreadable { error: String, last_lines: String },
  /// The program did not finish within the suite's timeout and was stopped with everything it started.
  TimedOut { timeout_secs: u64 },
}

/// A file a suite left, copied into the run's storage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredArtifact {
  /// Where the suite left it, relative to the worktree with `/` between its parts.
  pub source: String,
  /// The copy in the run's storage, absolute with `/` between its parts.
  pub stored: String,
  /// The test ([`test_name`]) whose report attached it; `None` for a file one of the suite's artifact globs found.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub test: Option<String>,
}

/// One `run_tests` call, kept with the run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuiteRun {
  pub id: TestRunId,
  /// The suite's slug.
  pub suite: String,
  pub filter: Option<String>,
  /// The program's exit code; `None` when it was stopped or ended by a signal.
  pub exit_code: Option<i32>,
  pub duration_ms: u64,
  pub end: SuiteRunEnd,
  pub artifacts: Vec<StoredArtifact>,
  /// Files the suite left that could not be copied, and why.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub artifact_problems: Vec<String>,
}

impl SuiteRun {
  /// Whether the suite passed: its report was read, it shows no failure, and the program exited with 0.
  pub fn passed(&self) -> bool {
    matches!(&self.end, SuiteRunEnd::Read { report } if report.passed()) && self.exit_code == Some(0)
  }

  /// The report, once it was read.
  pub const fn report(&self) -> Option<&TestReport> {
    match &self.end {
      SuiteRunEnd::Read { report } => Some(report),
      SuiteRunEnd::Unreadable { .. } | SuiteRunEnd::TimedOut { .. } => None,
    }
  }

  /// The stored copies of what the report attached to the test named `name`.
  pub fn artifacts_of<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> {
    self
      .artifacts
      .iter()
      .filter(move |artifact| artifact.test.as_deref() == Some(name))
      .map(|artifact| artifact.stored.as_str())
  }
}

/// The name `run_tests` gives a test and `test_output` takes: its suites and its own name, joined by ` > `.
pub fn test_name(case: &TestCase) -> String {
  case
    .suite()
    .iter()
    .map(String::as_str)
    .chain([case.name()])
    .collect::<Vec<_>>()
    .join(" > ")
}

/// The last [`LAST_LINES`] lines of a program's `stdout` and `stderr`, each cut to [`EXCERPT_LINE_CHARS`], without
/// terminal colours.
pub fn last_lines(stdout: &str, stderr: &str) -> String {
  let mut printed = excerpt::plain(stdout);
  if !stderr.trim().is_empty() {
    if !printed.is_empty() && !printed.ends_with('\n') {
      printed.push('\n');
    }
    printed.push_str(&excerpt::plain(stderr));
  }
  let lines: Vec<&str> = printed.lines().filter(|line| !line.trim().is_empty()).collect();
  let start = lines.len().saturating_sub(LAST_LINES);
  excerpt::lines(lines[start..].iter().copied(), LAST_LINES, EXCERPT_LINE_CHARS)
}
