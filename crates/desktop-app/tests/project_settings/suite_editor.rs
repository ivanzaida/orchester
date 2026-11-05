//! A test suite's page under Project settings › Services (`04 · Test suite editor` `uxYcD`): it opens with its fields,
//! what it needs started in order and that no run's results are kept yet, saves its draft as a field is left or a
//! choice made, says the core's errors in place without saving, makes a new suite on its first save, publishes the
//! draft, and puts back what an MCP client typed into what it runs. Run with
//! `cargo test -p orchester-app --test project_settings suite_editor`.

use std::collections::BTreeMap;

use lurq::app::events::with_synthetic_input;
use orchester_core::{
  resource::{
    ArtifactGlob, CommandArg, CommandProgram, CommandTimeout, ReportFormat, Slug, SuiteCommand, SuiteNeeds, SuitePath,
    SuiteVariable, SuiteVariables, TestLayer, TestReport, TestSuite, VariableName,
  },
  service::{Readiness, ReadyCheck, ReadyTimeout, Service, ServiceName, Stack, StackName},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_workspace::agent_testing::data::{create, entries, service_slug, stack_slug};

use super::{harness::Settings, service_editor::type_and_leave};

fn name(value: &str) -> ServiceName {
  ServiceName::new(value).expect("a kebab-case name")
}

fn service(value: &str, needs: &[&str]) -> Service {
  let readiness = Readiness::new(ReadyCheck::PortOpen, ReadyTimeout::from_secs(30).expect("in range"));
  Service::new(name(value), CommandProgram::new("npm").expect("a program"), readiness)
    .with_needs(needs.iter().map(|need| name(need)).collect())
    .expect("needs other services once each")
}

fn web_e2e() -> TestSuite {
  let args = ["playwright", "test", "--grep={{filter}}", "--reporter=json"]
    .map(|arg| CommandArg::new(arg).expect("an argument"))
    .to_vec();
  let command = SuiteCommand::new(CommandProgram::new("npx").expect("a program"), args).expect("a command");
  let report = TestReport::new(ReportFormat::PlaywrightJson, None).expect("a printed report");
  let variables = SuiteVariables::new(BTreeMap::from([(
    VariableName::new("BASE_URL").expect("a name"),
    SuiteVariable::Plain("{{services.cloud-web.url}}".to_owned()),
  )]))
  .expect("variables");
  TestSuite::new(command, report, TestLayer::E2e)
    .with_working_folder(Some(SuitePath::new("web/cloud").expect("a folder")))
    .with_variables(variables)
    .with_needs(SuiteNeeds::new(vec![Slug::new("cloud").expect("a slug")]).expect("needs"))
    .with_artifacts(vec![ArtifactGlob::new("test-results/**/*.png").expect("a glob")])
    .with_timeout(CommandTimeout::from_secs(600).expect("in range"))
}

/// `postgres` and `cloud-api` that needs it, published; the stack `cloud` of `cloud-api`; the suite `web-e2e`, a draft.
fn seed(settings: &Settings) {
  let mut db = settings.db();
  let now = Timestamp::now();
  for body in [service("postgres", &[]), service("cloud-api", &["postgres"])] {
    let id = create(&mut db, service_slug(body.name()), &body, now).expect("create a service");
    db.publish_draft::<Service>(id, now).expect("publish");
  }
  let cloud = StackName::new("cloud").expect("a name");
  let stack = Stack::new(cloud.clone(), vec![name("cloud-api")]).expect("a stack");
  create(&mut db, stack_slug(&cloud), &stack, now).expect("create a stack");
  create(&mut db, Slug::new("web-e2e").expect("a slug"), &web_e2e(), now).expect("create a suite");
}

/// The page of `web-e2e`, opened from its row.
fn open_suite() -> Settings {
  let mut settings = Settings::new(false);
  seed(&settings);
  settings.tab("services", "settings-suite-web-e2e-row");
  settings.click("settings-suite-web-e2e-row");
  settings.until("the suite's page", |s| s.exists("settings-suite-program"));
  settings
}

/// The suite `value` as stored now: its draft if it has one, otherwise its latest version.
fn stored(settings: &Settings, value: &str) -> Option<TestSuite> {
  entries::<TestSuite, _>(&settings.db())
    .expect("suites")
    .into_iter()
    .find(|entry| &*entry.name == value)
    .map(|entry| entry.body)
}

fn arguments(suite: &TestSuite) -> Vec<&str> {
  suite.command().args().iter().map(CommandArg::as_str).collect()
}

#[test]
fn suite_editor_opens_with_its_fields_needs_and_no_runs_yet() {
  let mut settings = open_suite();
  assert_eq!(
    settings.text("settings-suite-editor-crumb"),
    "Services / Test suites / web-e2e"
  );
  assert_eq!(settings.text("settings-suite-editor-head-title"), "web-e2e");
  assert!(!settings.exists("settings-suite-name"), "a suite keeps its name");
  assert_eq!(settings.value("settings-suite-program"), "npx");
  assert_eq!(
    settings.value("settings-suite-arguments"),
    "playwright\ntest\n--reporter=json"
  );
  assert_eq!(settings.value("settings-suite-filter"), "--grep={{filter}}");
  assert_eq!(settings.value("settings-suite-folder"), "web/cloud");
  assert_eq!(settings.value("settings-suite-variable-0-name"), "BASE_URL");
  assert_eq!(settings.value("settings-suite-timeout"), "10");
  assert_eq!(
    settings.text("settings-suite-chain"),
    "1 postgres 2 cloud-api · 2 slots"
  );
  assert!(settings.exists("settings-suite-artifact-0"));
  assert!(
    !settings.exists("settings-suite-report-path"),
    "Playwright json is read from its output"
  );
  assert_eq!(
    settings.text("settings-suite-last-runs-none"),
    "No run's results are kept yet. Once the Test step stores them, its last runs show here."
  );
  assert_eq!(settings.text("settings-suite-tool-0"), "run_tests web-e2e");
  assert_eq!(
    settings.text("settings-suite-tool-1"),
    "run_tests web-e2e --filter <filter>"
  );
  assert_eq!(settings.text("settings-suite-state"), "Draft · not published yet");
  assert_eq!(settings.text("project-settings-save-status"), "All changes saved");
}

#[test]
fn suite_editor_saves_the_draft_as_a_field_is_left_or_a_choice_made() {
  let mut settings = open_suite();
  type_and_leave(&mut settings, ("settings-suite-timeout", "15"), "settings-suite-filter");
  settings.until("the timeout stored", |s| {
    stored(s, "web-e2e").is_some_and(|suite| suite.timeout().secs() == 900)
  });
  settings.saved();
  assert_eq!(
    arguments(&stored(&settings, "web-e2e").expect("web-e2e")),
    ["playwright", "test", "--grep={{filter}}", "--reporter=json"],
    "the filter keeps its place among the arguments"
  );

  settings.click("settings-suite-layer-unit");
  settings.until("the layer stored", |s| {
    stored(s, "web-e2e").is_some_and(|suite| suite.layer() == TestLayer::Unit)
  });
  settings.click("settings-suite-always-switch");
  settings.until("always stored", |s| {
    stored(s, "web-e2e").is_some_and(|suite| suite.always())
  });
  settings.click("settings-suite-needs-nothing");
  settings.until("no needs stored", |s| {
    stored(s, "web-e2e").is_some_and(|suite| suite.needs().as_slice().is_empty())
  });
  settings.saved();
  assert!(!settings.exists("settings-suite-chain"));
}

#[test]
fn suite_editor_says_the_cores_errors_in_place_and_saves_nothing() {
  let mut settings = open_suite();
  settings.click("settings-suite-format-junit_xml");
  settings.until("the report's error", |s| s.exists("settings-suite-report-error"));
  assert!(
    settings.exists("settings-suite-report-path"),
    "JUnit XML is read from a file"
  );
  assert_eq!(
    settings.text("settings-suite-report-error-text"),
    "Enter the file the report is written to, inside the working folder."
  );
  assert_eq!(
    settings.text("project-settings-save-status"),
    "Not saved · fix the marked field"
  );
  assert_eq!(
    stored(&settings, "web-e2e").expect("web-e2e").report().format(),
    ReportFormat::PlaywrightJson,
    "nothing was saved"
  );
  type_and_leave(
    &mut settings,
    ("settings-suite-report-path", "results/junit.xml"),
    "settings-suite-filter",
  );
  settings.until("the report stored", |s| {
    stored(s, "web-e2e").is_some_and(|suite| suite.report().format() == ReportFormat::JunitXml)
  });
  settings.saved();

  type_and_leave(
    &mut settings,
    ("settings-suite-filter", "--grep"),
    "settings-suite-timeout",
  );
  settings.until("the filter's error", |s| s.exists("settings-suite-filter-note"));
  assert_eq!(
    settings.text("settings-suite-filter-note-text"),
    "Write the argument that takes run_tests' filter with {{filter}} in it, such as --grep={{filter}}, or leave it empty."
  );
  type_and_leave(
    &mut settings,
    ("settings-suite-filter", "--grep={{filter}}"),
    "settings-suite-timeout",
  );
  type_and_leave(
    &mut settings,
    ("settings-suite-arguments", "playwright\n--port={{port}}"),
    "settings-suite-timeout",
  );
  settings.until("the arguments' error", |s| s.exists("settings-suite-arguments-note"));
  assert_eq!(
    settings.text("settings-suite-arguments-note-text"),
    "{{port}} is not a placeholder of a test suite: the only one is {{filter}}, in the filter's argument."
  );
  assert_eq!(
    arguments(&stored(&settings, "web-e2e").expect("web-e2e")),
    ["playwright", "test", "--grep={{filter}}", "--reporter=json"],
    "nothing was saved"
  );
}

#[test]
fn new_suite_is_made_on_its_first_save_and_opens_under_its_id() {
  let mut settings = Settings::new(false);
  seed(&settings);
  settings.tab("services", "settings-suites-new");
  settings.click("settings-suites-new");
  settings.until("New suite's page", |s| s.exists("settings-suite-name"));
  assert_eq!(settings.text("settings-suite-state"), "Not saved yet");
  settings.set("settings-suite-name", "lint");
  type_and_leave(
    &mut settings,
    ("settings-suite-program", "cargo"),
    "settings-suite-arguments",
  );
  settings.until("the new suite's page", |s| {
    s.exists("settings-suite-program") && !s.exists("settings-suite-name")
  });
  settings.saved();
  let lint = stored(&settings, "lint").expect("lint is made");
  assert_eq!(lint.command().program().as_str(), "cargo");
  assert_eq!(settings.text("settings-suite-editor-head-title"), "lint");
  assert_eq!(settings.text("settings-suite-state"), "Draft · not published yet");
}

#[test]
fn suite_editor_publishes_the_draft() {
  let mut settings = open_suite();
  settings.click("settings-suite-publish");
  settings.until("published", |s| s.text("settings-suite-state") == "Published v1");
  let db = settings.db();
  let id = entries::<TestSuite, _>(&db)
    .expect("suites")
    .into_iter()
    .find(|entry| &*entry.name == "web-e2e")
    .expect("web-e2e")
    .id;
  assert!(db.latest_version::<TestSuite>(id).expect("versions").is_some());
  assert!(db.draft::<TestSuite>(id).expect("draft").is_none());
  assert!(!settings.exists("settings-suite-publish"));
}

/// An MCP client's program is put back and not saved while *Let automation change agent permissions* is off; its
/// Publish is refused.
#[test]
fn suite_editor_puts_back_what_an_mcp_client_typed_into_the_program() {
  let mut settings = open_suite();
  with_synthetic_input(|| {
    type_and_leave(
      &mut settings,
      ("settings-suite-program", "other-tool"),
      "settings-suite-arguments",
    );
  });
  settings.until("the refusal", |s| s.exists("settings-suite-person-only"));
  assert_eq!(settings.value("settings-suite-program"), "npx", "put back");
  assert_eq!(
    stored(&settings, "web-e2e")
      .expect("web-e2e")
      .command()
      .program()
      .as_str(),
    "npx"
  );
  with_synthetic_input(|| settings.click("settings-suite-publish"));
  for _ in 0..10 {
    settings.window.tree.tick_futures();
    settings.window.pass();
  }
  assert_eq!(settings.text("settings-suite-state"), "Draft · not published yet");
}

/// A person's program left while its save is under way is kept when an MCP client's arguments are put back during that
/// save: they are put back to the suite being saved, not to what was stored before it.
#[test]
fn suite_editor_keeps_a_persons_change_when_an_mcp_clients_is_put_back_during_its_save() {
  let mut settings = open_suite();
  type_and_leave(
    &mut settings,
    ("settings-suite-program", "pnpx"),
    "settings-suite-arguments",
  );
  assert_eq!(
    settings.text("project-settings-save-status"),
    "Saving…",
    "the person's save is under way"
  );
  with_synthetic_input(|| {
    type_and_leave(
      &mut settings,
      ("settings-suite-arguments", "other\nargs"),
      "settings-suite-folder",
    );
  });
  assert_eq!(
    settings.value("settings-suite-program"),
    "pnpx",
    "the person's program stays"
  );
  assert_eq!(
    settings.value("settings-suite-arguments"),
    "playwright\ntest\n--reporter=json",
    "the client's arguments are put back"
  );
  settings.until("the refusal", |s| s.exists("settings-suite-person-only"));
  settings.until("the person's program stored", |s| {
    stored(s, "web-e2e").is_some_and(|suite| suite.command().program().as_str() == "pnpx")
  });
  settings.saved();
  let suite = stored(&settings, "web-e2e").expect("web-e2e");
  assert_eq!(suite.command().program().as_str(), "pnpx", "the save after it keeps it");
  assert_eq!(
    arguments(&suite),
    ["playwright", "test", "--grep={{filter}}", "--reporter=json"]
  );
  assert_eq!(settings.value("settings-suite-program"), "pnpx");
}

/// An MCP client cannot make a new suite while *Let automation change agent permissions* is off.
#[test]
fn new_suite_is_not_made_by_an_mcp_client() {
  let mut settings = Settings::new(false);
  seed(&settings);
  settings.tab("services", "settings-suites-new");
  settings.click("settings-suites-new");
  settings.until("New suite's page", |s| s.exists("settings-suite-name"));
  with_synthetic_input(|| {
    settings.set("settings-suite-name", "lint");
    type_and_leave(
      &mut settings,
      ("settings-suite-program", "cargo"),
      "settings-suite-arguments",
    );
  });
  settings.until("the refusal", |s| s.exists("settings-suite-person-only"));
  assert!(stored(&settings, "lint").is_none(), "nothing was made");
  assert!(settings.exists("settings-suite-name"), "still the New suite page");
}
