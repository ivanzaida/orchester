//! The Test step end to end against `orchester-fake-codex`, with `orchester-test-command` as the suites' and services'
//! program (`support::suites`, `support::services`): its suites run layer by layer and it stops after the first layer
//! that fails; only the suites whose services own a path the run changed run, beside those marked always; a test that
//! fails and then passes when run again is flaky and does not fail the step; a failure returns to the writing step
//! with only the failed tests, never the raw output; and the step after it reads the result it stored. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test test_step test_step`.

mod support;

use std::{fs, path::Path};

use orchester_core::{
  id::{IdempotencyKey, RunId},
  resource::{ReportFormat, Slug, StepKey, SuiteNeeds, TestLayer, TestSuite},
  run::{Run, RunStatus, StepFailure, StepOutcome, StepOutput},
  service::{OwnedPath, ReadyCheck, Service},
  store::ActivityStore,
  test_report::SuiteRun,
};
use orchester_store::ProjectDb;

use self::support::{
  StepSpec, reported, scheduler,
  services::{publish, publish_one, report_file, service, service_project},
  settled,
  suites::{NODE_TAP, commit, layered},
  wait_for,
};

/// A TAP report in which the tests that fail in `NODE_TAP` pass.
const PASSING_TAP: &str = "TAP version 13\nok 1 - adds numbers\nok 2 - subtracts numbers\n1..2\n";

/// Implement writes `implement.txt`; Tests runs the suites and returns to Implement once; Review runs after it.
fn pipeline() -> Vec<StepSpec> {
  vec![
    StepSpec::new("implement", "Implement", "Write own file.", &[]),
    StepSpec::new("tests", "Tests", "", &["implement"])
      .tests()
      .retry("implement", 1),
    StepSpec::new("review", "Review", "Write own file.", &["tests"]),
  ]
}

/// The stand-in server `name`, owning the repository paths `owns`.
fn owning(name: &str, owns: &str) -> Service {
  service(
    name,
    &["listen".to_owned(), "{{port}}".to_owned()],
    ReadyCheck::PortOpen,
    20,
  )
  .with_owned_paths(vec![OwnedPath::new(owns).expect("a path")])
  .expect("owned paths")
}

/// `suite` needing the services or stacks `needs`.
fn needing(suite: TestSuite, needs: &[&str]) -> TestSuite {
  let needs = needs.iter().map(|need| Slug::new(need).expect("slug")).collect();
  suite.with_needs(SuiteNeeds::new(needs).expect("needs"))
}

/// A suite of `layer` printing the TAP report `fixture`.
fn printing(layer: TestLayer, fixture: &str) -> TestSuite {
  layered(layer, &["cat", fixture], (ReportFormat::Tap, None))
}

async fn run_to_the_end(root: &Path, task: orchester_core::id::TaskId) -> Run {
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), task)
    .await
    .expect("the run starts");
  wait_for(root, &mut events, id, settled).await
}

/// The test runs the run kept, in the order they started.
fn kept_runs(root: &Path, run: RunId) -> Vec<SuiteRun> {
  let tests = orchester_core::workspaces::artifacts_folder(root, run).join("tests");
  (1..)
    .map(|number| tests.join(format!("t{number}")).join("result.json"))
    .take_while(|file| file.exists())
    .map(|file| serde_json::from_slice(&fs::read(file).expect("kept")).expect("reads"))
    .collect()
}

fn outputs_of(db: &ProjectDb, run: RunId, step: &str) -> Vec<StepOutput> {
  let step = StepKey::new(step).expect("key");
  db.step_outputs(run)
    .expect("outputs")
    .into_iter()
    .filter(|output| output.step == step)
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn test_step_runs_changed_areas_layer_by_layer_and_returns_only_failed_tests() {
  let project = service_project(pipeline(), Vec::new());
  let root = project.repo.path();
  commit(
    root,
    &[("fixtures/node.tap", NODE_TAP), ("fixtures/pass.tap", PASSING_TAP)],
  );
  publish(root, &[owning("web", "implement.txt"), owning("api", "api/**")]);
  publish_one(
    root,
    "lint",
    &printing(TestLayer::Lint, "fixtures/pass.tap").run_always(true),
  );
  publish_one(
    root,
    "web-unit",
    &needing(printing(TestLayer::Unit, "fixtures/node.tap"), &["web"]),
  );
  publish_one(
    root,
    "api-unit",
    &needing(printing(TestLayer::Unit, "fixtures/pass.tap"), &["api"]),
  );
  publish_one(
    root,
    "e2e",
    &printing(TestLayer::E2e, "fixtures/pass.tap").run_always(true),
  );

  let run = run_to_the_end(root, project.task).await;
  assert_ne!(run.status(), &RunStatus::Succeeded);
  let failed: Vec<StepFailure> = run
    .attempts()
    .iter()
    .flat_map(|attempt| attempt.executions())
    .filter(|execution| execution.step().as_str() == "tests")
    .filter_map(|execution| execution.failure())
    .collect();
  assert_eq!(failed, [StepFailure::CommandFailed, StepFailure::CommandFailed]);

  // Lint first, then the unit suite whose service owns the changed file, its failed test once more; the suite of the
  // unchanged area and the e2e layer after the failing one never ran.
  let suites: Vec<String> = kept_runs(root, run.id()).into_iter().map(|kept| kept.suite).collect();
  assert_eq!(
    suites,
    ["lint", "web-unit", "web-unit", "lint", "web-unit", "web-unit"],
    "two attempts of the step"
  );

  let db = ProjectDb::open(root).expect("open");
  let output = &outputs_of(&db, run.id(), "tests")[0];
  assert_eq!(output.outcome, Some(StepOutcome::Failed));
  let text = &output.text;
  assert!(
    text.starts_with("The Test step failed at the unit layer: 1 tests failed"),
    "{text}"
  );
  assert!(text.contains("## unit · `web-unit`"), "{text}");
  assert!(
    text
      .lines()
      .any(|line| line.starts_with("- `subtracts numbers`: failed at ") && line.ends_with("math.test.mjs:8")),
    "{text}"
  );
  assert!(
    text.contains("  excerpt:\n    Expected values to be strictly equal:"),
    "{text}"
  );
  assert!(
    text.contains("Not run, after the failing layer: e2e (`e2e`)."),
    "{text}"
  );
  assert!(
    !text.contains("`lint`") && !text.contains("api-unit"),
    "only what failed: {text}"
  );
  // The test's own output and the program's are never in it.
  assert!(!text.contains("subtracting 5 - 3") && !text.contains("pid: "), "{text}");

  // Implement, the route's target, reads only the failed tests, as data.
  let prompts = reported(&db, run.id(), "prompt| ");
  assert!(
    prompts
      .iter()
      .any(|line| line == "# Returned from Tests (`tests`), attempt 1"),
    "{prompts:?}"
  );
  assert!(
    prompts
      .iter()
      .any(|line| line.starts_with("> - `subtracts numbers`: failed at ")),
    "{prompts:?}"
  );
  assert!(
    !prompts
      .iter()
      .any(|line| line.contains("subtracting 5 - 3") || line.starts_with("> pid: ") || line.contains("TAP version")),
    "{prompts:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn test_step_flaky_test_does_not_fail_and_review_reads_the_stored_result() {
  let project = service_project(pipeline(), Vec::new());
  let root = project.repo.path();
  commit(
    root,
    &[("fixtures/node.tap", NODE_TAP), ("fixtures/pass.tap", PASSING_TAP)],
  );
  let (marker, filter) = (report_file(root, "printed-once"), report_file(root, "filter.txt"));
  let flaky = layered(
    TestLayer::Unit,
    &[
      "first-then",
      &marker,
      "fixtures/node.tap",
      "fixtures/pass.tap",
      "write",
      &filter,
      "{{filter}}",
    ],
    (ReportFormat::Tap, None),
  )
  .run_always(true);
  publish_one(root, "unit", &flaky);

  let run = run_to_the_end(root, project.task).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert_eq!(run.attempts().len(), 1, "the flaky test sent nothing back");
  // The failed test ran once more with its filter, and passed.
  let kept = kept_runs(root, run.id());
  assert_eq!(kept.len(), 2);
  assert!(!kept[0].passed() && kept[1].filter.as_deref() == Some("subtracts numbers"));
  assert_eq!(
    fs::read_to_string(&filter).expect("the filter the rerun got"),
    "subtracts numbers"
  );

  let db = ProjectDb::open(root).expect("open");
  let output = &outputs_of(&db, run.id(), "tests")[0];
  assert_eq!(output.outcome, Some(StepOutcome::Done));
  assert!(
    output.text.starts_with("The Test step passed: 1 suites ran"),
    "{}",
    output.text
  );
  assert!(
    output.text.contains(
      "Flaky, so they do not fail the step: they failed, then passed when run again: `subtracts numbers` in `unit`."
    ),
    "{}",
    output.text
  );

  // Review reads the stored result, and how to check a single test.
  let prompts = reported(&db, run.id(), "prompt| ");
  assert!(prompts.iter().any(|line| line == "# Test results"), "{prompts:?}");
  assert!(
    prompts
      .iter()
      .any(|line| line.contains("call run_tests with the suite and a filter")),
    "{prompts:?}"
  );
  assert!(
    prompts.iter().any(|line| line == "## Tests (`tests`, attempt 1)"),
    "{prompts:?}"
  );
  assert!(
    prompts
      .iter()
      .any(|line| line.contains("`subtracts numbers` in `unit`")),
    "{prompts:?}"
  );
}
