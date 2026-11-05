//! `run_tests` and `test_output` end to end against `orchester-fake-codex`, with `orchester-test-command` as the suites'
//! program (`support::suites`): a suite's report in each format is read into the totals and each failure with its
//! message, file:line and excerpt, never the raw output; the services a suite needs start before it and stop after it;
//! the files it left are copied into the run's storage and returned as paths; and a report that cannot be read is an
//! explicit error with the exit code and the last lines, never a pass. Needs `git`.
//! Run with `cargo test -p orchester-runner --features test-agent --test run_tests run_tests`.

mod support;

use std::{fs, path::Path};

use orchester_core::{
  id::IdempotencyKey,
  resource::{ArtifactGlob, ReportFormat, Slug, SuiteNeeds},
  run::RunStatus,
  service::ReadyCheck,
  test_report::SuiteRunEnd,
};
use orchester_store::ProjectDb;

use self::support::{
  StepSpec, is_running,
  qa::{approve, calls, gone},
  reported, scheduler,
  services::{answered, publish, publish_one, report_file, report_folder, reported_pid, service, service_project},
  suites::{CARGO, NODE_JUNIT, NODE_TAP, PLAYWRIGHT, VITEST, answer, commit, stored, suite},
  wait_for,
};

fn implement(instructions: &'static str) -> Vec<StepSpec> {
  vec![StepSpec::new("implement", "Implement", instructions, &[])]
}

/// The answers of the run's `run_tests` calls, in order.
fn run_answers(root: &Path, run: orchester_core::id::RunId) -> Vec<String> {
  calls(root, run)
    .into_iter()
    .filter(|call| call.starts_with("run_tests"))
    .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn run_tests_reads_each_report_format_into_failures_without_raw_output() {
  let project = service_project(
    implement(
      "Run the suite junit. Run the suite vitest. Run the suite playwright. Run the suite tap. Run the suite cargo. \
       Then read the output of tests > subtracts.",
    ),
    Vec::new(),
  );
  let root = project.repo.path();
  commit(
    root,
    &[
      ("fixtures/cargo.json", CARGO),
      ("fixtures/node.junit.xml", NODE_JUNIT),
      ("fixtures/vitest.json", VITEST),
      ("fixtures/playwright.json", PLAYWRIGHT),
      ("fixtures/node.tap", NODE_TAP),
    ],
  );
  let copied = |fixture: &str, to: &str, format: ReportFormat| suite(&["copy", fixture, to], format, Some(to));
  publish_one(
    root,
    "junit",
    &copied("fixtures/node.junit.xml", "target/junit.xml", ReportFormat::JunitXml),
  );
  publish_one(
    root,
    "vitest",
    &copied("fixtures/vitest.json", "target/vitest.json", ReportFormat::VitestJson),
  );
  publish_one(
    root,
    "playwright",
    &copied("fixtures/playwright.json", "target/playwright.json", ReportFormat::PlaywrightJson),
  );
  publish_one(root, "tap", &suite(&["cat", "fixtures/node.tap"], ReportFormat::Tap, None));
  publish_one(root, "cargo", &suite(&["cat", "fixtures/cargo.json"], ReportFormat::CargoJson, None));
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  // Every run of a suite asks: suites have no approval rules of their own yet.
  for _ in 0..5 {
    approve(&scheduler, root, &mut events, id).await;
  }
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let answers = run_answers(root, id);
  assert_eq!(answers.len(), 5, "{answers:?}");
  // Suite, failed and errored tests, a failure's name ending and its location's ending.
  let expected = [
    ("junit", 1, "subtracts numbers", r"math.test.mjs"),
    ("vitest", 1, "subtracts", "math.test.ts:12"),
    ("playwright", 1, "pays with a card", "cart.spec.ts:14"),
    ("tap", 1, "subtracts numbers", ":8"),
    ("cargo", 2, "tests > subtracts", r"src\lib.rs:21"),
  ];
  for (call, (name, failed, failure, location)) in answers.iter().zip(expected) {
    let result = answer(call);
    assert_eq!(result["suite"], name);
    assert_eq!(result["passed"], false, "{name}");
    assert_eq!(result["exit_code"], 0, "{name}");
    let totals = &result["totals"];
    assert_eq!(
      totals["failed"].as_u64().unwrap_or(0) + totals["errored"].as_u64().unwrap_or(0),
      failed,
      "{name}: {totals}"
    );
    let failures = result["failures"].as_array().expect("failures");
    let found = failures
      .iter()
      .find(|shown| shown["name"].as_str().is_some_and(|shown| shown.ends_with(failure)))
      .unwrap_or_else(|| panic!("{name} lists {failure}: {result}"));
    assert!(
      found["location"].as_str().is_some_and(|shown| shown.ends_with(location)),
      "{name}: {found}"
    );
    assert!(found["message"].as_str().is_some_and(|message| !message.is_empty()), "{name}: {found}");
    assert!(found["excerpt"].is_string(), "{name}: {found}");
    // The program's own output and the tests' captured output stay out of the answer.
    assert!(!call.contains("pid: "), "{name}: {call}");
    assert!(!stored(&result).passed(), "{name}");
  }
  assert!(!answers[4].contains("subtracting 5 - 3"), "{}", answers[4]);
  assert!(!answers[2].contains("paying 12.50"), "{}", answers[2]);

  let output = calls(root, id)
    .into_iter()
    .find(|call| call.starts_with("test_output"))
    .expect("test_output answered");
  assert_eq!(output, r"test_output ok: tests > subtracts: failed at src\lib.rs:21");
  let lines = reported(&ProjectDb::open(root).expect("open"), id, "orchester-answer| ");
  assert!(
    lines.iter().any(|line| line.starts_with("subtracting 5 - 3")),
    "test_output gives the test's captured output: {lines:?}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn run_tests_starts_needed_services_before_and_stops_them_after() {
  let project = service_project(
    implement("Run the suite tap with filter subtracts. Hold the services."),
    Vec::new(),
  );
  let root = project.repo.path();
  commit(root, &[("fixtures/node.tap", NODE_TAP)]);
  let db_service = service(
    "db",
    &[
      "pid-file".to_owned(),
      report_file(root, "db.pid"),
      "listen".to_owned(),
      "{{port}}".to_owned(),
    ],
    ReadyCheck::PortOpen,
    20,
  );
  publish(root, &[db_service]);
  // The suite copies the pid `db` wrote when it started: without `db` running first it fails before its report.
  let seen = report_file(root, "seen.pid");
  let filter = report_file(root, "filter.txt");
  let pid = report_file(root, "db.pid");
  let tap = suite(
    &["copy", &pid, &seen, "write", &filter, "{{filter}}", "cat", "fixtures/node.tap"],
    ReportFormat::Tap,
    None,
  )
  .with_needs(SuiteNeeds::new(vec![Slug::new("db").expect("slug")]).expect("needs"));
  publish_one(root, "tap", &tap);
  let hold = root.join(".orchester").join("fake-hold");
  fs::write(&hold, "hold").expect("hold");
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;

  let result = answer(&answered(root, &mut events, id, "run_tests").await);
  assert_eq!(result["filter"], "subtracts");
  assert_eq!(result["totals"]["failed"], 1);
  let db_pid = reported_pid(root, "db.pid").await;
  assert_eq!(
    fs::read_to_string(report_folder(root).join("seen.pid")).expect("the suite saw db"),
    db_pid,
    "db ran before the suite"
  );
  assert_eq!(
    fs::read_to_string(report_folder(root).join("filter.txt")).expect("the filter"),
    "subtracts"
  );
  // The step still works: db stopped because the suite ended, not because the turn did.
  gone(&db_pid, "the service the suite needed, after the suite").await;
  assert!(hold.exists(), "the step is still held");

  fs::remove_file(&hold).expect("release");
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);
  assert!(!is_running(&db_pid));
}

#[tokio::test(flavor = "multi_thread")]
async fn run_tests_keeps_artifacts_with_the_run_and_returns_their_paths() {
  let project = service_project(implement("Run the suite e2e."), Vec::new());
  let root = project.repo.path();
  let results = "test-results/cart-checkout-pays-with-a-card-chromium";
  let report = PLAYWRIGHT.replace("/work/shop/test-results/", "test-results/");
  commit(
    root,
    &[
      ("fixtures/playwright.json", &report),
      ("fixtures/shot.png", "a screenshot"),
      ("fixtures/trace.zip", "a trace"),
    ],
  );
  let (shot, trace) = (format!("{results}/test-failed-1.png"), format!("{results}/trace.zip"));
  let e2e = suite(
    &[
      "copy",
      "fixtures/shot.png",
      &shot,
      "copy",
      "fixtures/trace.zip",
      &trace,
      "copy",
      "fixtures/playwright.json",
      "target/playwright.json",
    ],
    ReportFormat::PlaywrightJson,
    Some("target/playwright.json"),
  )
  .with_artifacts(vec![ArtifactGlob::new("test-results/**/*.png").expect("a glob")]);
  publish_one(root, "e2e", &e2e);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let result = answer(&run_answers(root, id)[0]);
  // The run's storage keeps the first test run in `tests/t1/`, its copies at their paths in the worktree.
  let folder = orchester_core::workspaces::artifacts_folder(root, id).join("tests").join("t1");
  let shown = |path: &Path| path.to_string_lossy().replace('\\', "/");
  let kept = |relative: &str| shown(&relative.split('/').fold(folder.join("artifacts"), |path, part| path.join(part)));
  // The glob found the screenshot; the report attached the screenshot and the trace to the failure.
  assert_eq!(result["artifacts"], serde_json::json!([kept(&shot), kept(&trace)]));
  let failure = &result["failures"][0];
  assert!(failure["name"].as_str().is_some_and(|name| name.ends_with("pays with a card")));
  assert_eq!(failure["artifacts"], serde_json::json!([kept(&shot), kept(&trace)]));
  assert_eq!(fs::read_to_string(kept(&shot)).expect("kept"), "a screenshot");
  assert_eq!(fs::read_to_string(kept(&trace)).expect("kept"), "a trace");
  assert_eq!(result["result"], shown(&folder.join("result.json")));
  assert_eq!(stored(&result).artifacts.len(), 3, "the glob's copy and the failure's two");
}

#[tokio::test(flavor = "multi_thread")]
async fn run_tests_unreadable_report_is_an_error_never_a_pass() {
  let project = service_project(implement("Run the suite missing. Run the suite cut."), Vec::new());
  let root = project.repo.path();
  // Writes no report and says why on standard error.
  let missing = suite(
    &["stderr", "the runner crashed", "exit", "3"],
    ReportFormat::JunitXml,
    Some("target/junit.xml"),
  );
  // Prints a TAP test line without the plan, then exits with 0.
  let cut = suite(&["say", "TAP version 13", "say", "ok 1 - adds"], ReportFormat::Tap, None);
  publish_one(root, "missing", &missing);
  publish_one(root, "cut", &cut);
  let (scheduler, mut events) = scheduler(root);
  let id = scheduler
    .start_run(IdempotencyKey::generate(), project.task)
    .await
    .expect("run");
  approve(&scheduler, root, &mut events, id).await;
  approve(&scheduler, root, &mut events, id).await;
  let run = wait_for(root, &mut events, id, |run| run.status().is_terminal()).await;
  assert_eq!(run.status(), &RunStatus::Succeeded);

  let answers = run_answers(root, id);
  assert!(
    answers[0].starts_with(
      "run_tests error: the suite `missing` did not pass: its report cannot be read (the program wrote no report at"
    ),
    "{}",
    answers[0]
  );
  assert!(answers[0].contains("; exit code 3. Its last lines:"), "{}", answers[0]);
  assert!(
    answers[1].starts_with("run_tests error: the suite `cut` did not pass: its report cannot be read (the TAP report"),
    "{}",
    answers[1]
  );
  assert!(answers[1].contains("exit code 0"), "{}", answers[1]);
  let lines = reported(&ProjectDb::open(root).expect("open"), id, "orchester-answer| ");
  assert!(lines.iter().any(|line| line == "the runner crashed"), "{lines:?}");
  // Each is kept with the run as unreadable.
  let tests = orchester_core::workspaces::artifacts_folder(root, id).join("tests");
  for run in ["t1", "t2"] {
    let kept: orchester_core::test_report::SuiteRun =
      serde_json::from_slice(&fs::read(tests.join(run).join("result.json")).expect("kept")).expect("reads");
    assert!(matches!(kept.end, SuiteRunEnd::Unreadable { .. }), "{run}");
    assert!(!kept.passed());
  }
}
