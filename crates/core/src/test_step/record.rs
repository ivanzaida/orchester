//! What a Test step found, kept with the run for the task page's Tests card (`docs/architecture/agent-testing.md`,
//! "The Test step"; `07 · Summary · Tests` `qN2FK`): each layer with its suites' counts and times, each failed test
//! with its file:line, the start of its message and its artifacts, the flaky tests, the quarantined ones that failed
//! again, and every file the suites left. It is JSON in the run's storage, one file per execution of the step
//! ([`record_file`]), beside the suites' own runs: what the person reads, never the programs' raw output.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{FailedTest, SuiteOutcome, TestStepResult, flaky::Quarantine};
use crate::{
  id::StepExecutionId,
  resource::TestLayer,
  test_report::{StoredArtifact, SuiteRun, TestRunId, Totals, test_name},
};

/// The folder of the run's storage the records go to.
const RECORDS: &str = "test-steps";

/// The file the record of the Test step's `execution` is kept in, in the run's storage `storage`.
pub fn record_file(storage: &Path, execution: StepExecutionId) -> PathBuf {
  storage.join(RECORDS).join(format!("{execution}.json"))
}

/// A Test step's result as the task page shows it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestStepRecord {
  /// Every layer of the step's plan in order: those it ran, then those it did not reach after one failed.
  pub layers: Vec<LayerRecord>,
  /// The suites the run's changes left out.
  pub left_out: Vec<String>,
}

/// One layer of a Test step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerRecord {
  #[serde(with = "layer_name")]
  pub layer: TestLayer,
  /// Whether it ran: a layer after one that failed does not.
  pub ran: bool,
  pub suites: Vec<SuiteRecord>,
}

/// One suite of a layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuiteRecord {
  /// Its slug.
  pub suite: String,
  /// Its first run, kept as `run_tests` keeps one; `None` when it did not run.
  pub run: Option<TestRunId>,
  /// How long its first run took.
  pub duration_ms: u64,
  /// The totals of its first run, once its report was read.
  pub totals: Option<Totals>,
  /// The tests still failing after they ran again, and not quarantined.
  pub failed: Vec<TestRecord>,
  pub flaky: Vec<FlakyRecord>,
  /// The quarantined tests that failed when run again too.
  pub quarantined: Vec<TestRecord>,
  /// Why it fails besides its failed tests.
  pub problem: Option<String>,
  /// The files its first run left, copied into the run's storage.
  pub artifacts: Vec<StoredArtifact>,
}

/// A test that failed, and failed again when run again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestRecord {
  /// Its suites and its own name, as `run_tests` names it.
  pub name: String,
  /// `file:line`, or the file alone.
  pub location: Option<String>,
  pub message: Option<String>,
  pub excerpt: Option<String>,
  /// The stored copies of what its report attached to it.
  pub artifacts: Vec<String>,
}

/// A test that failed, then passed when run again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlakyRecord {
  pub name: String,
  pub location: Option<String>,
  /// The person quarantined it: it would not have failed the step either way.
  pub quarantined: bool,
}

impl TestStepRecord {
  /// `result` as the task page shows it, its flaky tests marked when `quarantine` holds them.
  pub fn of(result: &TestStepResult, quarantine: &Quarantine) -> Self {
    let reached = result.reached.iter().map(|(layer, suites)| LayerRecord {
      layer: *layer,
      ran: true,
      suites: suites.iter().map(|suite| SuiteRecord::of(suite, quarantine)).collect(),
    });
    let not_reached = result.not_reached.iter().map(|(layer, suites)| LayerRecord {
      layer: *layer,
      ran: false,
      suites: suites.iter().map(|slug| SuiteRecord::not_run(slug.as_str())).collect(),
    });
    Self {
      layers: reached.chain(not_reached).collect(),
      left_out: result.left_out.iter().map(|slug| slug.as_str().to_owned()).collect(),
    }
  }

  pub fn suites(&self) -> impl Iterator<Item = &SuiteRecord> {
    self.layers.iter().flat_map(|layer| layer.suites.iter())
  }

  /// The tests that failed the step, in the order they ran.
  pub fn failed(&self) -> impl Iterator<Item = &TestRecord> {
    self.suites().flat_map(|suite| suite.failed.iter())
  }

  pub fn flaky(&self) -> impl Iterator<Item = &FlakyRecord> {
    self.suites().flat_map(|suite| suite.flaky.iter())
  }

  pub fn quarantined(&self) -> impl Iterator<Item = &TestRecord> {
    self.suites().flat_map(|suite| suite.quarantined.iter())
  }

  /// Every file the suites' first runs left.
  pub fn artifacts(&self) -> impl Iterator<Item = &StoredArtifact> {
    self.suites().flat_map(|suite| suite.artifacts.iter())
  }
}

impl LayerRecord {
  /// Whether every suite of a layer that ran passed.
  pub fn passed(&self) -> bool {
    self.ran && self.suites.iter().all(SuiteRecord::passed)
  }

  /// The tests its suites' first runs passed.
  pub fn passed_tests(&self) -> usize {
    self.totals(|totals| totals.passed)
  }

  /// The tests of its suites still failing, not quarantined.
  pub fn failed_tests(&self) -> usize {
    self.suites.iter().map(|suite| suite.failed.len()).sum()
  }

  pub fn flaky_tests(&self) -> usize {
    self.suites.iter().map(|suite| suite.flaky.len()).sum()
  }

  /// Its suites that failed besides their tests: they did not run, their report could not be read, they timed out.
  pub fn suites_with_problems(&self) -> usize {
    self.suites.iter().filter(|suite| suite.problem.is_some()).count()
  }

  /// How long its suites' first runs took together.
  pub fn duration_ms(&self) -> u64 {
    self.suites.iter().map(|suite| suite.duration_ms).sum()
  }

  fn totals(&self, count: impl Fn(&Totals) -> usize) -> usize {
    self
      .suites
      .iter()
      .filter_map(|suite| suite.totals.as_ref())
      .map(count)
      .sum()
  }
}

impl SuiteRecord {
  fn of(outcome: &SuiteOutcome, quarantine: &Quarantine) -> Self {
    let first = outcome.kept.as_ref().map(|(run, _)| run);
    let flaky = outcome
      .flaky
      .iter()
      .map(|name| FlakyRecord {
        location: first.and_then(|run| location_of(run, name)),
        quarantined: quarantine.holds(&outcome.suite, name),
        name: name.clone(),
      })
      .collect();
    Self {
      suite: outcome.suite.as_str().to_owned(),
      run: first.map(|run| run.id),
      duration_ms: first.map_or(0, |run| run.duration_ms),
      totals: outcome.totals(),
      failed: outcome.failed.iter().map(TestRecord::of).collect(),
      flaky,
      quarantined: outcome.quarantined.iter().map(TestRecord::of).collect(),
      problem: outcome.problem.clone(),
      artifacts: first.map(|run| run.artifacts.clone()).unwrap_or_default(),
    }
  }

  fn not_run(suite: &str) -> Self {
    Self {
      suite: suite.to_owned(),
      run: None,
      duration_ms: 0,
      totals: None,
      failed: Vec::new(),
      flaky: Vec::new(),
      quarantined: Vec::new(),
      problem: None,
      artifacts: Vec::new(),
    }
  }

  pub fn passed(&self) -> bool {
    self.failed.is_empty() && self.problem.is_none()
  }
}

impl TestRecord {
  fn of(test: &FailedTest) -> Self {
    Self {
      name: test.name.clone(),
      location: test.location.clone(),
      message: test.message.clone(),
      excerpt: test.excerpt.clone(),
      artifacts: test.artifacts.clone(),
    }
  }
}

/// Where the test `name` of `run`'s report is, as `file:line`.
fn location_of(run: &SuiteRun, name: &str) -> Option<String> {
  let case = run.report()?.tests().iter().find(|case| test_name(case) == name)?;
  let location = case.location()?;
  Some(match location.line {
    Some(line) => format!("{}:{line}", location.file),
    None => location.file.clone(),
  })
}

/// What kind of file a test left, by its name: what the task page offers to open it as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtifactKind {
  Screenshot,
  /// A recorded trace, as Playwright writes it: a `.zip` or a file named for a trace.
  Trace,
  Video,
  Log,
  Other,
}

impl ArtifactKind {
  pub const ALL: [Self; 5] = [Self::Screenshot, Self::Trace, Self::Video, Self::Log, Self::Other];

  /// The kind of the file at `path`.
  pub fn of(path: &str) -> Self {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_ascii_lowercase();
    let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
    match extension {
      "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => Self::Screenshot,
      "zip" => Self::Trace,
      _ if name.contains("trace") => Self::Trace,
      "webm" | "mp4" | "mov" => Self::Video,
      "log" | "txt" => Self::Log,
      _ => Self::Other,
    }
  }

  /// Whether a press may hand the file to its default program: a picture, an archive, a video or a text, which
  /// that program shows. Any other file a suite left could be a program or a script, which opening would run: its
  /// folder opens instead.
  pub const fn opens_itself(self) -> bool {
    !matches!(self, Self::Other)
  }
}

/// [`TestLayer`] as its name, `lint`, `unit` or `e2e`.
mod layer_name {
  use serde::{Deserialize, Deserializer, Serializer, de::Error};

  use crate::resource::TestLayer;

  pub fn serialize<S: Serializer>(layer: &TestLayer, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(layer.as_str())
  }

  pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<TestLayer, D::Error> {
    TestLayer::parse(&String::deserialize(deserializer)?).map_err(D::Error::custom)
  }
}
