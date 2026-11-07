//! `flaky-tests`: Project settings › Services › Flaky tests (`09 · Flaky tests` `Tv9VC`) on a real flaky history. The
//! project has the `services` scenario's services and suites, and five runs of *Keep the e2e suites green* whose Test
//! steps saw tests flaky: three are quarantined (`billing › shows the invoice list` in five runs, `invites › the invite
//! email arrives` in three, `store::tests::concurrent_writes` in four), two are not (`teams › renames a team` in two
//! runs today, `auth::tests::refresh_expires` in one twelve days ago). Open Project settings › Services › Flaky tests.

use orchester_core::{
  id::{IdempotencyKey, RunId},
  integration::base::RunBaseSource,
  resource::Slug,
  run::{BranchName, NewRun},
  store::RunStore,
  test_step::flaky::{FlakyKind, FlakyOutcome, TestKey},
  time::Timestamp,
};

use super::services;
use crate::kontur::{Kontur, SeedError, catalogue};

const TITLE: &str = "Keep the e2e suites green";
const BRIEF: &str = "Find why the billing and invite e2e tests fail now and then, and fix them.";

const MINUTES_A_DAY: i64 = 24 * 60;

fn invalid(error: impl std::fmt::Display) -> SeedError {
  SeedError::Invalid(error.to_string())
}

fn key(suite: &str, test: &str) -> Result<TestKey, SeedError> {
  TestKey::new(Slug::new(suite).map_err(invalid)?, test).map_err(invalid)
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  services::seed(kontur)?;
  let published = kontur.ago(30 * MINUTES_A_DAY);
  let connection = catalogue::codex(&mut kontur.db, kontur.fake_codex.as_deref(), published)?;
  let pipeline = catalogue::tested(&mut kontur.db, connection, published)?;
  let task = kontur.task((TITLE, BRIEF, &[]), Some(&pipeline), published)?;
  let mut runs: Vec<RunId> = Vec::new();
  for earlier in 0..5 {
    let new = NewRun {
      task: task.id(),
      snapshot: pipeline.snapshot().clone(),
      branch: BranchName::for_next_run(task.key(), earlier),
      base: kontur.base.clone(),
      base_branch: None,
      base_source: RunBaseSource::CheckedOut,
      budget: None,
      max_duration: None,
    };
    let started = kontur.ago((14 - 3 * i64::try_from(earlier).map_err(invalid)?) * MINUTES_A_DAY);
    runs.push(kontur.db.create_run(IdempotencyKey::generate(), new, started)?.id());
  }
  // Each test with the runs (by index) it was flaky in and how many minutes ago each was; the last is when it was
  // last seen.
  let history: [(TestKey, &[(usize, i64)]); 5] = [
    (
      key("web-e2e", "billing › shows the invoice list")?,
      &[
        (0, 13 * MINUTES_A_DAY),
        (1, 10 * MINUTES_A_DAY),
        (2, 7 * MINUTES_A_DAY),
        (3, 4 * MINUTES_A_DAY),
        (4, 120),
      ],
    ),
    (
      key("team-e2e", "invites › the invite email arrives")?,
      &[(1, 10 * MINUTES_A_DAY), (2, 7 * MINUTES_A_DAY), (4, MINUTES_A_DAY + 60)],
    ),
    (
      key("rust-unit", "store::tests::concurrent_writes")?,
      &[
        (0, 13 * MINUTES_A_DAY),
        (1, 12 * MINUTES_A_DAY + 60),
        (2, 12 * MINUTES_A_DAY + 30),
        (3, 12 * MINUTES_A_DAY),
      ],
    ),
    (key("web-e2e", "teams › renames a team")?, &[(3, 30), (4, 10)]),
    (
      key("rust-unit", "auth::tests::refresh_expires")?,
      &[(1, 12 * MINUTES_A_DAY)],
    ),
  ];
  let mut outcomes = Vec::new();
  for (test, seen) in &history {
    for (run, minutes) in *seen {
      outcomes.push(FlakyOutcome {
        test: test.clone(),
        kind: FlakyKind::Flaky,
        run: runs[*run],
        at: kontur.ago(*minutes),
      });
    }
  }
  kontur.db.record_flaky_outcomes(&outcomes)?;
  let quarantined: Timestamp = kontur.ago(16 * MINUTES_A_DAY);
  for (test, _) in &history[..3] {
    kontur.db.quarantine_test(test, quarantined)?;
  }
  kontur.seeded(
    &task,
    "five runs whose Test steps saw five tests flaky; three of them quarantined",
  );
  Ok(())
}
