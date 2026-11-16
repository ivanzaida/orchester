//! Flaky tests and quarantine (`docs/architecture/agent-testing.md`, decision 4 and "Flaky tests and quarantine"): each
//! flaky outcome a Test step sees is kept for the project ([`FlakyOutcome`]); a test flaky in [`WATCH_AFTER`] runs
//! within [`WATCH_WINDOW_MILLIS`] is *watched* ([`Standing::Watched`]); the person moves a test to quarantine and back
//! ([`Quarantine`]). A quarantined test still runs; its failure is kept and shown, but never fails the step.
//!
//! Why three runs in fourteen days: one flaky outcome is often chance (a loaded computer, a slow disk), and two can be
//! a coincidence of the same bad hour; a third in another run is a pattern worth the person's look. Runs are counted,
//! not outcomes, so one run whose Test step goes back and forth does not make a test watched alone. Two weeks spans
//! several working days of runs in an active project, and a test that was fixed leaves the watched list on its own
//! after two quiet weeks.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{id::RunId, resource::Slug, time::Timestamp};

/// How many runs a test must be flaky in, within [`WATCH_WINDOW_MILLIS`], to be watched.
pub const WATCH_AFTER: usize = 3;

/// The window, back from now, in which a test's flaky runs count toward [`WATCH_AFTER`]: fourteen days.
pub const WATCH_WINDOW_MILLIS: i64 = 14 * 24 * 60 * 60 * 1000;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TestKeyError {
  #[error("a test's name must not be blank")]
  Blank,
}

/// A test of a project as Test steps name it: its suite and its name as `run_tests` names it
/// ([`crate::test_report::test_name`]).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestKey {
  suite: Slug,
  test: String,
}

impl TestKey {
  pub fn new(suite: Slug, test: &str) -> Result<Self, TestKeyError> {
    if test.trim().is_empty() {
      return Err(TestKeyError::Blank);
    }
    Ok(Self {
      suite,
      test: test.to_owned(),
    })
  }

  pub const fn suite(&self) -> &Slug {
    &self.suite
  }

  pub fn test(&self) -> &str {
    &self.test
  }
}

/// What a Test step saw of a test it keeps for the project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlakyKind {
  /// It failed, then passed when run again.
  Flaky,
  /// It is quarantined and failed when run again too: shown, not failing the step.
  QuarantinedFailure,
}

impl FlakyKind {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Flaky => "flaky",
      Self::QuarantinedFailure => "quarantined_failure",
    }
  }

  pub fn parse(text: &str) -> Option<Self> {
    match text {
      "flaky" => Some(Self::Flaky),
      "quarantined_failure" => Some(Self::QuarantinedFailure),
      _ => None,
    }
  }
}

/// One outcome a Test step kept: which test, what it saw, in which run and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlakyOutcome {
  pub test: TestKey,
  pub kind: FlakyKind,
  pub run: RunId,
  pub at: Timestamp,
}

/// The project's quarantined tests: their failures never fail a Test step.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Quarantine(BTreeSet<TestKey>);

impl Quarantine {
  pub fn new(tests: impl IntoIterator<Item = TestKey>) -> Self {
    Self(tests.into_iter().collect())
  }

  /// Whether the test `test` of suite `suite` is quarantined.
  pub fn holds(&self, suite: &Slug, test: &str) -> bool {
    self.0.iter().any(|key| key.suite == *suite && key.test == test)
  }
}

/// Where a test with flaky outcomes stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
  /// The person quarantined it at `since`.
  Quarantined { since: Timestamp },
  /// It was flaky in [`WATCH_AFTER`] runs or more within the window.
  Watched,
  /// It was flaky, in fewer runs within the window.
  Seen,
}

/// A test as the flaky tests list shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlakyTest {
  pub test: TestKey,
  pub standing: Standing,
  /// The runs it was flaky in within the window.
  pub flaky_runs: usize,
  /// When it was last flaky; `None` for a quarantined test never seen flaky.
  pub last_flaky: Option<Timestamp>,
  /// When it last failed while quarantined.
  pub last_quarantined_failure: Option<Timestamp>,
}

#[derive(Default)]
struct Seen {
  runs: BTreeSet<RunId>,
  last_flaky: Option<Timestamp>,
  last_quarantined_failure: Option<Timestamp>,
}

/// Every test with a flaky outcome or in quarantine at `now`, from the project's `outcomes` and `quarantined` tests
/// (with when each was quarantined): quarantined first, then watched, then the others, each by suite and name.
pub fn flaky_tests(outcomes: &[FlakyOutcome], quarantined: &[(TestKey, Timestamp)], now: Timestamp) -> Vec<FlakyTest> {
  let since = now.unix_millis().saturating_sub(WATCH_WINDOW_MILLIS);
  let mut seen: BTreeMap<&TestKey, Seen> = BTreeMap::new();
  for outcome in outcomes {
    let entry = seen.entry(&outcome.test).or_default();
    let last = match outcome.kind {
      FlakyKind::Flaky => {
        if outcome.at.unix_millis() >= since {
          entry.runs.insert(outcome.run);
        }
        &mut entry.last_flaky
      }
      FlakyKind::QuarantinedFailure => &mut entry.last_quarantined_failure,
    };
    *last = (*last).max(Some(outcome.at));
  }
  for (test, _) in quarantined {
    seen.entry(test).or_default();
  }
  let mut tests: Vec<FlakyTest> = seen
    .into_iter()
    .map(|(test, seen)| {
      let standing = match quarantined.iter().find(|(key, _)| key == test) {
        Some((_, since)) => Standing::Quarantined { since: *since },
        None if seen.runs.len() >= WATCH_AFTER => Standing::Watched,
        None => Standing::Seen,
      };
      FlakyTest {
        test: test.clone(),
        standing,
        flaky_runs: seen.runs.len(),
        last_flaky: seen.last_flaky,
        last_quarantined_failure: seen.last_quarantined_failure,
      }
    })
    .collect();
  tests.sort_by(|a, b| {
    rank(a.standing)
      .cmp(&rank(b.standing))
      .then_with(|| a.test.cmp(&b.test))
  });
  tests
}

const fn rank(standing: Standing) -> u8 {
  match standing {
    Standing::Quarantined { .. } => 0,
    Standing::Watched => 1,
    Standing::Seen => 2,
  }
}
