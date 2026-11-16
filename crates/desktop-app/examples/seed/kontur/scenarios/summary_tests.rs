//! `summary-tests`: a task whose run's Test step failed twice, for the Tests card on its Summary (`07 · Summary ·
//! Tests` `qN2FK`). The run passed Explore, Plan and Implement; its Test step failed and sent the failed tests back to
//! Implement, which passed again; the Test step failed again in attempt 2 and sent them back once more, and Implement
//! works in attempt 3. Attempt 2's Test step kept its record in the run's storage, as the runner keeps one: lint
//! passed, unit passed with a flaky test, e2e failed two tests (each with a screenshot and a trace, real files in the
//! run's storage) and had a flaky test that is quarantined. Its report is the step's output too, which the card
//! replaces on the Summary. Open *Invite people to a team server*.

use std::{fs, path::Path};

use orchester_core::{
  resource::{StepKey, TestLayer},
  run::{StepFailure, StepOutcome, StepOutput},
  store::ActivityStore,
  test_report::{StoredArtifact, TestRunId, Totals},
  test_step::{FlakyRecord, LayerRecord, SuiteRecord, TestRecord, TestStepRecord, record_file},
  workspaces::artifacts_folder,
};

use crate::kontur::{Kontur, SeedError, catalogue, io_error, run::Drive};

const TITLE: &str = "Invite people to a team server";
const BRIEF: &str = "Let a team's admin invite people by email, and let an invite be accepted once.";
const CRITERIA: [&str; 2] = [
  "An invite accepted twice says it was accepted already.",
  "A renamed team keeps its members.",
];

/// A 1 × 1 PNG: a screenshot a failed e2e test left.
const SCREENSHOT: &[u8] = &[
  0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
  0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49,
  0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x60, 0x60, 0x60, 0xF8, 0x0F, 0x00, 0x01, 0x04, 0x01, 0x00, 0x5F, 0xE5, 0xC3,
  0x4B, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];
/// An empty zip archive: a Playwright trace.
const TRACE: &[u8] = &[
  0x50, 0x4B, 0x05, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  let published = kontur.ago(3 * 60);
  let connection = catalogue::codex(&mut kontur.db, kontur.fake_codex.as_deref(), published)?;
  let pipeline = catalogue::tested(&mut kontur.db, connection, published)?;
  let mut task = kontur.task((TITLE, BRIEF, &CRITERIA), Some(&pipeline), kontur.ago(150))?;
  kontur.queue(&mut task, &pipeline)?;
  let started = kontur.ago(90);
  let mut drive = Drive::start(kontur, &task, &pipeline, started)?;
  drive
    .pass_before("test")?
    .fail("test", StepFailure::CommandFailed)?
    .pass_before("test")?;
  let execution = drive.execution("test")?;
  drive.fail("test", StepFailure::CommandFailed)?.begin("implement")?;
  let run = drive.finish();
  let storage = artifacts_folder(&kontur.repo, run.id());
  let record = record(&storage)?;
  let file = record_file(&storage, execution);
  let json = serde_json::to_vec_pretty(&record).map_err(|error| SeedError::Invalid(error.to_string()))?;
  write(&file, &json)?;
  let created_at = kontur.ago(20);
  kontur.db.save_step_output(&StepOutput {
    run: run.id(),
    execution,
    step: StepKey::new("test").expect("a valid step key"),
    outcome: Some(StepOutcome::Failed),
    text: "The Test step failed at the e2e layer: 2 tests failed in 2 suites. Only what failed is listed.\n".to_owned(),
    unverified: Vec::new(),
    created_at,
  })?;
  kontur.seeded(
    &task,
    "attempt 2's Test step failed 2 e2e tests and sent them back; Implement works in attempt 3",
  );
  Ok(())
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), SeedError> {
  if let Some(folder) = path.parent() {
    fs::create_dir_all(folder).map_err(io_error(folder))?;
  }
  fs::write(path, bytes).map_err(io_error(path))
}

/// A failed test's screenshot and trace, written under the test run `run`'s artifacts in `storage`; their stored
/// paths.
fn artifacts(storage: &Path, run: &str, test: &str) -> Result<Vec<StoredArtifact>, SeedError> {
  let folder = storage
    .join("tests")
    .join(run)
    .join("artifacts")
    .join("test-results")
    .join(test);
  let mut stored = Vec::new();
  for (file, bytes) in [("test-failed-1.png", SCREENSHOT), ("trace.zip", TRACE)] {
    let path = folder.join(file);
    write(&path, bytes)?;
    stored.push(StoredArtifact {
      source: format!("test-results/{test}/{file}"),
      stored: path.to_string_lossy().replace('\\', "/"),
      test: Some(test.to_owned()),
    });
  }
  Ok(stored)
}

fn totals(passed: usize, failed: usize) -> Option<Totals> {
  Some(Totals {
    passed,
    failed,
    skipped: 0,
    errored: 0,
  })
}

fn run_id(text: &str) -> Result<TestRunId, SeedError> {
  text.parse().map_err(SeedError::Invalid)
}

fn suite(name: &str, (run, duration_ms): (&str, u64), totals: Option<Totals>) -> Result<SuiteRecord, SeedError> {
  Ok(SuiteRecord {
    suite: name.to_owned(),
    run: Some(run_id(run)?),
    duration_ms,
    totals,
    failed: Vec::new(),
    flaky: Vec::new(),
    quarantined: Vec::new(),
    problem: None,
    artifacts: Vec::new(),
  })
}

fn failed(name: &str, location: &str, message: &str, artifacts: &[StoredArtifact]) -> TestRecord {
  TestRecord {
    name: name.to_owned(),
    location: Some(location.to_owned()),
    message: Some(message.to_owned()),
    excerpt: Some(message.to_owned()),
    artifacts: artifacts.iter().map(|artifact| artifact.stored.clone()).collect(),
  }
}

fn flaky(name: &str, location: &str, quarantined: bool) -> FlakyRecord {
  FlakyRecord {
    name: name.to_owned(),
    location: Some(location.to_owned()),
    quarantined,
  }
}

/// Attempt 2's Test step, as the design shows it.
fn record(storage: &Path) -> Result<TestStepRecord, SeedError> {
  let lint = LayerRecord {
    layer: TestLayer::Lint,
    ran: true,
    suites: vec![
      suite("rust-lint", ("t4", 21_000), totals(1, 0))?,
      suite("web-lint", ("t5", 17_000), totals(1, 0))?,
    ],
  };
  let mut rust_unit = suite("rust-unit", ("t6", 81_000), totals(300, 1))?;
  rust_unit.flaky = vec![flaky(
    "store::tests::concurrent_writes",
    "crates/cloud/src/store/tests.rs:212",
    false,
  )];
  let unit = LayerRecord {
    layer: TestLayer::Unit,
    ran: true,
    suites: vec![rust_unit, suite("web-unit", ("t7", 44_000), totals(112, 0))?],
  };
  let invites = artifacts(storage, "t8", "invites-accepts-an-invite-twice")?;
  let mut web_e2e = suite("web-e2e", ("t8", 141_000), totals(30, 2))?;
  web_e2e.failed = vec![failed(
    "invites › accepts an invite twice",
    "web/team/e2e/invites.spec.ts:48",
    "Error: expect(received).toBe(expected)\n\nExpected: 409   Received: 500",
    &invites,
  )];
  web_e2e.flaky = vec![flaky(
    "billing › shows the invoice list",
    "web/cloud/e2e/billing.spec.ts:31",
    true,
  )];
  web_e2e.artifacts = invites;
  let teams = artifacts(storage, "t9", "teams-renames-a-team")?;
  let mut team_e2e = suite("team-e2e", ("t9", 96_000), totals(16, 1))?;
  team_e2e.failed = vec![failed(
    "teams › renames a team",
    "web/cloud/e2e/teams.spec.ts:112",
    "TimeoutError: locator.click: Timeout 10000ms exceeded\nwaiting for getByRole('button', { name: 'Save' })",
    &teams,
  )];
  team_e2e.artifacts = teams;
  let e2e = LayerRecord {
    layer: TestLayer::E2e,
    ran: true,
    suites: vec![web_e2e, team_e2e],
  };
  Ok(TestStepRecord {
    layers: vec![lint, unit, e2e],
    left_out: Vec::new(),
  })
}
