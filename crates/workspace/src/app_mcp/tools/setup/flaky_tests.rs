//! `list_flaky_tests`, `quarantine_test` and `release_test` (ORC-259): a project's flaky tests as Project settings ›
//! Services › Flaky tests reads them ([`crate::agent_testing::flaky`]), and the person's move of one to quarantine and
//! back. A quarantined test's failures pass the Test step, so quarantining is person_only unless a person lets
//! automation change agent permissions; releasing one holds runs to it again and is always taken.

use orchester_core::{
  resource::Slug,
  test_step::flaky::{FlakyTest, Standing, TestKey, WATCH_AFTER, WATCH_WINDOW_MILLIS},
  time::Timestamp,
};
use rmcp::{handler::server::common::schema_for_type, schemars::JsonSchema};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
  super::{Spec, parse, projects::InProject, wire},
  widening,
};
use crate::{
  agent_testing::flaky::{self, QuarantineError},
  app_mcp::{
    error::ToolError,
    services::{Services, blocking},
  },
  messages::{self, Message},
  tasks::outside::Outside,
};

const DAY_MILLIS: i64 = 24 * 60 * 60 * 1000;

pub(super) const SPECS: &[Spec] = &[
  Spec {
    name: "list_flaky_tests",
    description: "A project's flaky tests, as Project settings › Services › Flaky tests lists them: each test a Test \
                  step saw fail and then pass when run again, or that is quarantined, with its suite, its name as \
                  run_tests names it, standing (quarantined with quarantined_since, watched, or seen), flaky_runs \
                  (the runs it was flaky in within the watch window), last_flaky and last_quarantined_failure (ms \
                  since the epoch, null for never); quarantined first, then watched, then the others. A test flaky in \
                  watch.after_runs runs within watch.window_days days is watched. A quarantined test still runs; its \
                  failure is shown in the Test step's result but never fails the step.",
    schema: schema_for_type::<InProject>,
    read_only: true,
  },
  Spec {
    name: "quarantine_test",
    description: "Moves a flaky test to quarantine, as Flaky tests' Quarantine does: Test steps that start from then \
                  on still run it and show its failure, but it never fails the step and is never returned to the \
                  writing step. Only a test a Test step saw flaky is quarantined (not_flaky otherwise). It lets \
                  failures pass, so it is refused as person_only unless a person turned on Let automation change \
                  agent permissions. Answers whether it changed and the test as list_flaky_tests gives it.",
    schema: schema_for_type::<InTest>,
    read_only: false,
  },
  Spec {
    name: "release_test",
    description: "Takes a test out of quarantine, as Flaky tests' Release does: its failures fail the Test steps that \
                  start from then on again. Always taken. Answers whether it changed and the test as list_flaky_tests \
                  gives it, or null when it was never flaky.",
    schema: schema_for_type::<InTest>,
    read_only: false,
  },
];

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct InTest {
  /// The project's id, or its name.
  pub project: String,
  /// The test suite's name (its slug), as list_flaky_tests gives it.
  pub suite: String,
  /// The test's name as list_flaky_tests gives it.
  pub test: String,
}

/// Calls tool `name` of [`SPECS`] with `arguments`.
pub(super) async fn call(services: &Services, name: &str, arguments: Value) -> Result<Value, ToolError> {
  match name {
    "list_flaky_tests" => list_flaky_tests(services, parse(arguments)?).await,
    "quarantine_test" => quarantine_test(services, parse(arguments)?).await,
    "release_test" => release_test(services, parse(arguments)?).await,
    other => Err(ToolError::InvalidArguments(format!("no tool is named {other}"))),
  }
}

async fn list_flaky_tests(services: &Services, args: InProject) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let tests = flaky::read(&open.repo_root, Timestamp::now())?;
    Ok(json!({
      "tests": tests.iter().map(test_json).collect::<Vec<_>>(),
      "watch": { "after_runs": WATCH_AFTER, "window_days": WATCH_WINDOW_MILLIS / DAY_MILLIS },
    }))
  })
  .await
}

async fn quarantine_test(services: &Services, args: InTest) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let key = test_key(&args)?;
    let reason = Message::new("appMcp.flakyTests.quarantine")
      .with("test", key.test().to_owned())
      .with("suite", key.suite().as_str().to_owned());
    widening::allowed(&services, vec![reason])?;
    let now = Timestamp::now();
    let changed = flaky::quarantine(&open.repo_root, &key, now).map_err(|error| match error {
      QuarantineError::NotFlaky => ToolError::refused(
        "not_flaky",
        vec![
          Message::new("appMcp.flakyTests.notFlaky")
            .with("test", key.test().to_owned())
            .with("suite", key.suite().as_str().to_owned()),
        ],
      ),
      QuarantineError::Store(error) => ToolError::refused("project_data", vec![messages::store_error(&error)]),
    })?;
    services.changes.record(open.project.id(), Outside::ProjectSettings);
    Ok(json!({ "changed": changed, "test": current(&open.repo_root, &key, now)? }))
  })
  .await
}

async fn release_test(services: &Services, args: InTest) -> Result<Value, ToolError> {
  let services = services.clone();
  blocking(move || {
    let open = services.project(&args.project)?;
    let key = test_key(&args)?;
    let changed = flaky::release(&open.repo_root, &key)?;
    services.changes.record(open.project.id(), Outside::ProjectSettings);
    Ok(json!({ "changed": changed, "test": current(&open.repo_root, &key, Timestamp::now())? }))
  })
  .await
}

fn test_key(args: &InTest) -> Result<TestKey, ToolError> {
  let invalid = |reason: String| {
    ToolError::refused(
      "invalid_test",
      vec![Message::new("appMcp.flakyTests.invalid").with("reason", reason)],
    )
  };
  let suite = Slug::new(&args.suite).map_err(|error| invalid(error.to_string()))?;
  TestKey::new(suite, &args.test).map_err(|error| invalid(error.to_string()))
}

/// Test `key` as list_flaky_tests gives it now; null when no Test step saw it flaky and it is not quarantined.
fn current(root: &std::path::Path, key: &TestKey, now: Timestamp) -> Result<Value, ToolError> {
  let tests = flaky::read(root, now)?;
  Ok(
    tests
      .iter()
      .find(|test| test.test == *key)
      .map_or(Value::Null, test_json),
  )
}

fn test_json(test: &FlakyTest) -> Value {
  let (standing, since) = match test.standing {
    Standing::Quarantined { since } => ("quarantined", Some(wire::time(since))),
    Standing::Watched => ("watched", None),
    Standing::Seen => ("seen", None),
  };
  json!({
    "suite": test.test.suite().as_str(),
    "test": test.test.test(),
    "standing": standing,
    "quarantined_since": since,
    "flaky_runs": test.flaky_runs,
    "last_flaky": test.last_flaky.map(wire::time),
    "last_quarantined_failure": test.last_quarantined_failure.map(wire::time),
  })
}
