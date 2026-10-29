//! Test suites (ORC-244, `docs/architecture/agent-testing.md`, "Test suite"): every part is a validated
//! value, `{{filter}}` stands only where it can be filled, the report format and its path agree, and
//! a stored body is validated again when read. Run with
//! `cargo test -p orchester-core --test test_suite test_suite`.

use std::collections::BTreeMap;

use orchester_core::{
  resource::{
    ArtifactGlob, CommandArg, CommandError, CommandProgram, CommandTimeout, ReportFormat, ReportOutput, Slug,
    SuiteCommand, SuiteNeeds, SuitePath, SuiteVariable, SuiteVariables, TestFilter, TestLayer, TestReport, TestSuite,
    TestSuiteError, VariableName,
  },
  secret::SecretName,
};

fn args(values: &[&str]) -> Vec<CommandArg> {
  values
    .iter()
    .map(|value| CommandArg::new(value).expect("valid argument"))
    .collect()
}

fn command(values: &[&str]) -> Result<SuiteCommand, TestSuiteError> {
  SuiteCommand::new(CommandProgram::new("cargo").expect("valid program"), args(values))
}

fn path(value: &str) -> SuitePath {
  SuitePath::new(value).expect("valid path")
}

fn slug(value: &str) -> Slug {
  Slug::new(value).expect("valid slug")
}

fn full_suite() -> TestSuite {
  let variables = SuiteVariables::new(BTreeMap::from([
    (
      VariableName::new("RUST_LOG").expect("name"),
      SuiteVariable::Plain("warn".into()),
    ),
    (
      VariableName::new("DATABASE_URL").expect("name"),
      SuiteVariable::Secret(SecretName::new("STAGING_DATABASE_URL").expect("secret name")),
    ),
  ]))
  .expect("valid variables");
  TestSuite::new(
    command(&["nextest", "run", "--profile", "ci", "-E", "test(/{{filter}}/)"]).expect("valid command"),
    TestReport::new(ReportFormat::JunitXml, Some(path("target/nextest/ci/junit.xml"))).expect("valid report"),
    TestLayer::E2e,
  )
  .with_working_folder(Some(path("web\\cloud")))
  .with_variables(variables)
  .with_needs(SuiteNeeds::new(vec![slug("cloud"), slug("team-web")]).expect("valid needs"))
  .with_artifacts(vec![ArtifactGlob::new("test-results/**/*.png").expect("valid glob")])
  .with_timeout(CommandTimeout::from_secs(1800).expect("valid timeout"))
  .run_always(true)
}

#[test]
fn test_suite_needs_a_program() {
  assert_eq!(
    CommandProgram::new("   ").map_err(TestSuiteError::from),
    Err(TestSuiteError::Command(CommandError::BlankProgram))
  );
  let stored = serde_json::json!({
    "program": "", "args": [], "report": {"format": "tap"}, "layer": "unit", "timeout_secs": 60, "always": false
  });
  let refused = serde_json::from_value::<TestSuite>(stored).expect_err("a body without a program is refused");
  assert!(refused.to_string().contains("needs a program"), "{refused}");
}

#[test]
fn test_suite_filter_placeholder_stands_once_in_one_argument() {
  assert!(command(&["test", "--", "{{filter}}"]).is_ok());
  assert!(command(&["test", "--grep={{filter}}"]).is_ok());
  assert!(command(&["test"]).is_ok(), "a suite may take no filter");
  assert_eq!(
    SuiteCommand::new(CommandProgram::new("{{filter}}").expect("program"), Vec::new()),
    Err(TestSuiteError::PlaceholderInProgram)
  );
  assert_eq!(
    command(&["{{filter}}", "{{filter}}"]),
    Err(TestSuiteError::RepeatedFilter)
  );
  assert_eq!(
    command(&["--grep={{filter}}|{{filter}}"]),
    Err(TestSuiteError::RepeatedFilter)
  );
  assert_eq!(
    command(&["{{ filter }}"]),
    Err(TestSuiteError::UnknownPlaceholder("{{ filter }}".into()))
  );
  assert_eq!(
    command(&["--port={{port}}"]),
    Err(TestSuiteError::UnknownPlaceholder("{{port}}".into()))
  );
  assert_eq!(
    command(&["--grep={{filter"]),
    Err(TestSuiteError::UnclosedPlaceholder("--grep={{filter".into()))
  );
}

#[test]
fn test_suite_filter_fills_its_argument_or_leaves_it_out() {
  let suite = command(&["test", "--grep={{filter}}", "--", "--nocapture"]).expect("valid command");
  assert!(suite.accepts_filter());
  let filter = TestFilter::new("skills migration").expect("valid filter");
  assert_eq!(
    suite.arguments(Some(&filter)),
    ["test", "--grep=skills migration", "--", "--nocapture"]
  );
  assert_eq!(suite.arguments(None), ["test", "--", "--nocapture"]);
  assert!(!command(&["test"]).expect("valid").accepts_filter());
  assert_eq!(TestFilter::new(""), Err(CommandError::EmptyArgument));
  assert!(matches!(
    TestFilter::new("a\nb"),
    Err(CommandError::ControlCharacter(_))
  ));
}

#[test]
fn test_suite_report_format_is_one_of_five_and_agrees_with_its_path() {
  for (name, format) in [
    ("cargo_json", ReportFormat::CargoJson),
    ("junit_xml", ReportFormat::JunitXml),
    ("vitest_json", ReportFormat::VitestJson),
    ("playwright_json", ReportFormat::PlaywrightJson),
    ("tap", ReportFormat::Tap),
  ] {
    assert_eq!(ReportFormat::parse(name), Ok(format));
    assert_eq!(format.as_str(), name);
  }
  assert_eq!(
    ReportFormat::parse("xunit"),
    Err(TestSuiteError::UnknownReportFormat("xunit".into()))
  );
  assert_eq!(
    ReportFormat::parse("JUnit_XML"),
    Err(TestSuiteError::UnknownReportFormat("JUnit_XML".into()))
  );
  assert_eq!(ReportFormat::JunitXml.output(), ReportOutput::File);
  assert_eq!(
    TestReport::new(ReportFormat::JunitXml, None),
    Err(TestSuiteError::ReportPathMissing(ReportFormat::JunitXml))
  );
  for printed in [ReportFormat::CargoJson, ReportFormat::Tap] {
    assert_eq!(
      TestReport::new(printed, Some(path("report.json"))),
      Err(TestSuiteError::ReportPrinted(printed))
    );
    assert_eq!(TestReport::new(printed, None).expect("printed").path(), None);
  }
  for either in [ReportFormat::VitestJson, ReportFormat::PlaywrightJson] {
    assert!(TestReport::new(either, None).is_ok());
    let file = TestReport::new(either, Some(path("results.json"))).expect("written");
    assert_eq!(file.path(), Some(&path("results.json")));
  }
}

#[test]
fn test_suite_paths_stay_inside_the_worktree() {
  assert_eq!(path("web\\cloud/").as_str(), "web/cloud/");
  assert_eq!(SuitePath::new("  "), Err(TestSuiteError::BlankPath));
  for absolute in ["/etc/report.xml", "\\report.xml", "C:\\report.xml", "C:report.xml"] {
    assert_eq!(
      SuitePath::new(absolute),
      Err(TestSuiteError::AbsolutePath(absolute.into())),
      "{absolute}"
    );
  }
  assert_eq!(
    SuitePath::new("target/../../out.xml"),
    Err(TestSuiteError::PathLeavesWorktree("target/../../out.xml".into()))
  );
  assert_eq!(
    ArtifactGlob::new("..\\**\\*.png"),
    Err(TestSuiteError::PathLeavesWorktree("..\\**\\*.png".into()))
  );
  assert!(matches!(
    ArtifactGlob::new("shots/\n*.png"),
    Err(TestSuiteError::Command(CommandError::ControlCharacter(_)))
  ));
}

#[test]
fn test_suite_variables_needs_and_layers_are_validated() {
  let twice = SuiteVariables::new(BTreeMap::from([
    (
      VariableName::new("Path").expect("name"),
      SuiteVariable::Plain("a".into()),
    ),
    (
      VariableName::new("PATH").expect("name"),
      SuiteVariable::Plain("b".into()),
    ),
  ]));
  assert_eq!(
    twice,
    Err(TestSuiteError::Command(CommandError::DuplicateVariable("Path".into())))
  );
  let nul = SuiteVariables::new(BTreeMap::from([(
    VariableName::new("TOKEN").expect("name"),
    SuiteVariable::Plain("a\0b".into()),
  )]));
  assert_eq!(
    nul,
    Err(TestSuiteError::Command(CommandError::VariableValue("TOKEN".into())))
  );
  assert_eq!(
    SuiteNeeds::new(vec![slug("cloud"), slug("cloud")]),
    Err(TestSuiteError::RepeatedNeed("cloud".into()))
  );
  assert_eq!(TestLayer::parse("e2e"), Ok(TestLayer::E2e));
  assert_eq!(
    TestLayer::parse("integration"),
    Err(TestSuiteError::UnknownLayer("integration".into()))
  );
  assert!(TestLayer::Lint < TestLayer::Unit && TestLayer::Unit < TestLayer::E2e);
}

#[test]
fn test_suite_round_trips_and_is_validated_again_when_read() {
  let suite = full_suite();
  let stored = serde_json::to_value(&suite).expect("serialize");
  assert_eq!(
    stored["report"],
    serde_json::json!({"format": "junit_xml", "path": "target/nextest/ci/junit.xml"})
  );
  assert_eq!(stored["layer"], "e2e");
  assert_eq!(stored["working_folder"], "web/cloud");
  assert_eq!(
    stored["variables"]["DATABASE_URL"],
    serde_json::json!({"secret": "STAGING_DATABASE_URL"})
  );
  assert_eq!(stored["needs"], serde_json::json!(["cloud", "team-web"]));
  assert_eq!(stored["always"], true);
  assert_eq!(
    serde_json::from_value::<TestSuite>(stored.clone()).expect("read"),
    suite
  );

  for (pointer, value) in [
    ("/report/format", serde_json::json!("xunit")),
    ("/report/path", serde_json::Value::Null),
    ("/layer", serde_json::json!("smoke")),
    ("/args/5", serde_json::json!("{{filter}}-{{filter}}")),
    ("/needs/1", serde_json::json!("cloud")),
    ("/working_folder", serde_json::json!("../elsewhere")),
    ("/variables/DATABASE_URL", serde_json::json!({"secret": "lower case"})),
    ("/timeout_secs", serde_json::json!(0)),
  ] {
    let mut broken = stored.clone();
    *broken.pointer_mut(pointer).expect("field") = value;
    assert!(
      serde_json::from_value::<TestSuite>(broken).is_err(),
      "{pointer} is validated when read"
    );
  }
}

#[test]
fn test_suite_defaults_run_in_the_worktree_only_when_owned_paths_changed() {
  let suite = TestSuite::new(
    command(&["clippy"]).expect("valid command"),
    TestReport::new(ReportFormat::CargoJson, None).expect("valid report"),
    TestLayer::Lint,
  );
  assert_eq!(suite.working_folder(), None);
  assert!(suite.variables().is_empty());
  assert!(suite.needs().as_slice().is_empty());
  assert!(suite.artifacts().is_empty());
  assert_eq!(suite.timeout(), CommandTimeout::DEFAULT);
  assert!(!suite.always());
  let stored = serde_json::to_value(&suite).expect("serialize");
  assert_eq!(
    stored,
    serde_json::json!({
      "program": "cargo", "args": ["clippy"], "report": {"format": "cargo_json"}, "layer": "lint",
      "timeout_secs": 600, "always": false
    })
  );
}
