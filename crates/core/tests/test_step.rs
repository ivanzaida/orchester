//! The Test step's rules (`orchester_core::test_step`): which suites run for the paths a run changed, in which layer
//! order; which failed tests run again and how; when a failed test is flaky; and what the writing step reads when
//! the step fails — only the failed tests, never a test's output.

use orchester_core::{
  resource::{
    CommandArg, CommandProgram, ReportFormat, Slug, SuiteCommand, SuiteNeeds, TestLayer, TestReport, TestSuite,
  },
  service::{OwnedPath, Readiness, ReadyCheck, ReadyTimeout, Service, ServiceName, Stack, StackName},
  test_report::{self, SuiteRun, SuiteRunEnd, TestRunId},
  test_step::{Chosen, MAX_SINGLE_RERUNS, SuiteOutcome, TestPlan, TestStepResult, flaky::Quarantine, rerun_filters},
};

const FAILING: &str = "TAP version 13\n# printed by subtracts\nok 1 - adds\nnot ok 2 - subtracts\n  ---\n  \
                       location: 'src/math.test.mjs:8:1'\n  error: |-\n    8 !== 2\n  ...\n1..2\n";
const PASSING: &str = "TAP version 13\nok 1 - adds\nok 2 - subtracts\n1..2\n";

fn slug(value: &str) -> Slug {
  Slug::new(value).expect("slug")
}

fn suite(layer: TestLayer, needs: &[&str], filter: bool) -> TestSuite {
  let mut arguments = vec![CommandArg::new("test").expect("argument")];
  if filter {
    arguments.push(CommandArg::new("{{filter}}").expect("argument"));
  }
  let command = SuiteCommand::new(CommandProgram::new("cargo").expect("program"), arguments).expect("command");
  let needs = needs.iter().map(|need| slug(need)).collect();
  TestSuite::new(
    command,
    TestReport::new(ReportFormat::Tap, None).expect("report"),
    layer,
  )
  .with_needs(SuiteNeeds::new(needs).expect("needs"))
}

fn service(name: &str, owns: &[&str], needs: &[&str]) -> Service {
  let readiness = Readiness::new(ReadyCheck::PortOpen, ReadyTimeout::from_secs(5).expect("timeout"));
  Service::new(
    ServiceName::new(name).expect("name"),
    CommandProgram::new("server").expect("program"),
    readiness,
  )
  .with_owned_paths(owns.iter().map(|path| OwnedPath::new(path).expect("path")).collect())
  .expect("owned paths")
  .with_needs(needs.iter().map(|need| ServiceName::new(need).expect("name")).collect())
  .expect("needs")
}

/// The test run `id` that read `report`, its program exiting with 0 when it passed and 1 when not.
fn run(id: u32, report: &str) -> SuiteRun {
  let mut number = TestRunId::FIRST;
  for _ in 1..id {
    number = number.next();
  }
  let report = test_report::read_report(test_report::ReportFormat::Tap, report).expect("a TAP report");
  SuiteRun {
    id: number,
    suite: "unit".to_owned(),
    filter: None,
    exit_code: Some(if report.passed() { 0 } else { 1 }),
    duration_ms: 10,
    end: SuiteRunEnd::Read { report },
    artifacts: Vec::new(),
    artifact_problems: Vec::new(),
  }
}

#[test]
fn test_step_plan_runs_the_changed_areas_and_always_by_layer() {
  let services = [
    service("api", &["server/**"], &[]),
    service("web", &["web"], &["api"]),
    service("docs", &["docs/**"], &[]),
  ];
  let stacks = [Stack::new(
    StackName::new("full").expect("name"),
    vec![ServiceName::new("web").expect("name")],
  )
  .expect("stack")];
  let suites = vec![
    (slug("e2e"), suite(TestLayer::E2e, &["full"], false)),
    (slug("web-unit"), suite(TestLayer::Unit, &["web"], false)),
    (slug("docs-unit"), suite(TestLayer::Unit, &["docs"], false)),
    (slug("api-unit"), suite(TestLayer::Unit, &["api"], false)),
    (slug("lint"), suite(TestLayer::Lint, &[], false).run_always(true)),
    (slug("format"), suite(TestLayer::Lint, &[], false)),
  ];
  let plan = TestPlan::new(suites, &services, &stacks, &["server/src/main.rs".to_owned()]);
  let layers: Vec<(TestLayer, Vec<&str>)> = plan
    .layers()
    .iter()
    .map(|layer| {
      (
        layer.layer,
        layer.suites.iter().map(|planned| planned.slug.as_str()).collect(),
      )
    })
    .collect();
  // `web` needs `api`, which owns the changed file; the stack `full` holds `web`.
  assert_eq!(
    layers,
    [
      (TestLayer::Lint, vec!["lint"]),
      (TestLayer::Unit, vec!["api-unit", "web-unit"]),
      (TestLayer::E2e, vec!["e2e"]),
    ]
  );
  assert_eq!(plan.layers()[0].suites[0].chosen, Chosen::Always);
  assert_eq!(
    plan.layers()[1].suites[0].chosen,
    Chosen::Owns {
      service: ServiceName::new("api").expect("name"),
      path: "server/src/main.rs".to_owned(),
    }
  );
  assert_eq!(plan.left_out(), [slug("docs-unit"), slug("format")]);
  // A pattern without wildcards owns the folder it names, not a file that only starts like it.
  let web_only = TestPlan::new(
    vec![(slug("web-unit"), suite(TestLayer::Unit, &["web"], false))],
    &services,
    &stacks,
    &["website/index.html".to_owned(), "web/app.ts".to_owned()],
  );
  assert_eq!(web_only.layers().len(), 1);
  let unchanged = TestPlan::new(
    vec![(slug("web-unit"), suite(TestLayer::Unit, &["web"], false))],
    &services,
    &stacks,
    &["website/index.html".to_owned()],
  );
  assert!(unchanged.layers().is_empty());
}

#[test]
fn test_step_failed_tests_run_again_by_name_or_with_the_whole_suite() {
  let failing = run(1, FAILING);
  let filters = rerun_filters(&suite(TestLayer::Unit, &[], true), &failing);
  assert_eq!(
    filters
      .iter()
      .map(|filter| filter.as_ref().map(|filter| filter.as_str()))
      .collect::<Vec<_>>(),
    [Some("subtracts")]
  );
  // Without a filter the suite runs once more whole; so it does past the most single reruns.
  assert_eq!(rerun_filters(&suite(TestLayer::Unit, &[], false), &failing), [None]);
  let many: String = (1..=MAX_SINGLE_RERUNS + 1)
    .map(|number| format!("not ok {number} - case {number}\n"))
    .collect();
  let many = run(1, &format!("TAP version 13\n{many}1..{}\n", MAX_SINGLE_RERUNS + 1));
  assert_eq!(rerun_filters(&suite(TestLayer::Unit, &[], true), &many), [None]);
  assert!(rerun_filters(&suite(TestLayer::Unit, &[], true), &run(1, PASSING)).is_empty());
}

#[test]
fn test_step_a_test_that_passes_again_is_flaky_and_does_not_fail_the_suite() {
  let none = Quarantine::default();
  let flaky = SuiteOutcome::settle(
    slug("unit"),
    TestLayer::Unit,
    (run(1, FAILING), "t1/result.json".to_owned()),
    &[run(2, PASSING)],
    &none,
  );
  assert!(flaky.passed(), "{flaky:?}");
  assert_eq!(flaky.flaky, ["subtracts"]);
  let failing = SuiteOutcome::settle(
    slug("unit"),
    TestLayer::Unit,
    (run(1, FAILING), "t1/result.json".to_owned()),
    &[run(2, FAILING)],
    &none,
  );
  assert!(!failing.passed());
  assert_eq!(failing.failed[0].name, "subtracts");
  assert_eq!(failing.failed[0].location.as_deref(), Some("src/math.test.mjs:8"));
  // A rerun that did not run shows nothing passed.
  let unrun = SuiteOutcome::settle(
    slug("unit"),
    TestLayer::Unit,
    (run(1, FAILING), "t1".to_owned()),
    &[],
    &none,
  );
  assert!(!unrun.passed());
}

#[test]
fn test_step_failure_report_lists_only_the_failed_tests() {
  let none = Quarantine::default();
  let failing = SuiteOutcome::settle(
    slug("web-unit"),
    TestLayer::Unit,
    (run(2, FAILING), "/runs/r/tests/t2/result.json".to_owned()),
    &[run(3, FAILING)],
    &none,
  );
  let passing = SuiteOutcome::settle(
    slug("lint"),
    TestLayer::Lint,
    (run(1, PASSING), "/runs/r/tests/t1/result.json".to_owned()),
    &[],
    &none,
  );
  let missing = SuiteOutcome::not_run(
    slug("api-unit"),
    TestLayer::Unit,
    "the program was not found".to_owned(),
  );
  let result = TestStepResult {
    reached: vec![
      (TestLayer::Lint, vec![passing]),
      (TestLayer::Unit, vec![failing, missing]),
    ],
    not_reached: vec![(TestLayer::E2e, vec![slug("e2e")])],
    left_out: vec![slug("docs")],
  };
  assert!(!result.passed());
  assert_eq!(result.failed_layer(), Some(TestLayer::Unit));
  let report = result.failure_report();
  assert!(
    report.starts_with("The Test step failed at the unit layer: 1 tests failed in 2 suites."),
    "{report}"
  );
  assert!(
    report.contains("## unit · `web-unit` (test run t2, kept in /runs/r/tests/t2/result.json)"),
    "{report}"
  );
  assert!(
    report.contains("- `subtracts`: failed at src/math.test.mjs:8"),
    "{report}"
  );
  assert!(report.contains("  excerpt:\n    8 !== 2"), "{report}");
  assert!(
    report.contains("The suite did not pass: the program was not found."),
    "{report}"
  );
  assert!(
    report.contains("Not run, after the failing layer: e2e (`e2e`)."),
    "{report}"
  );
  assert!(report.contains("run_tests(suite, filter)"), "{report}");
  assert!(!report.contains("`lint`") && !report.contains("`adds`"), "{report}");
  assert!(
    !report.contains("printed by subtracts"),
    "a test's output is never in it: {report}"
  );
}
