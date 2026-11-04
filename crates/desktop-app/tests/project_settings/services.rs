//! Project settings › Services (`01 · Services` `G1sdS`, `01b · Services · none yet` `CdjWu`): the empty state with
//! Import draft and New service, the rows of what the project defines with their real facts, the pages a row, New
//! and Import draft open, which say they are not built yet, and `navigate` to them. Run with
//! `cargo test -p orchester-app --test project_settings services`.

use orchester_app::mcp::tools::navigate;
use orchester_core::{
  resource::{
    CommandArg, CommandProgram, CommandTimeout, ReportFormat, Slug, SuiteCommand, SuiteNeeds, TestLayer, TestReport,
    TestSuite,
  },
  service::{
    OwnedPath, Readiness, ReadyCheck, ReadyTimeout, RunData, Service, ServiceName, Stack, StackName, UiDriver,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_workspace::agent_testing::data::{create, service_slug, stack_slug};
use serde_json::json;

use super::harness::Settings;
use crate::support::{Window, project_id};

fn name(value: &str) -> ServiceName {
  ServiceName::new(value).expect("a kebab-case name")
}

fn service(value: &str, needs: &[&str]) -> Service {
  let readiness = Readiness::new(ReadyCheck::PortOpen, ReadyTimeout::from_secs(30).expect("in range"));
  Service::new(name(value), CommandProgram::new("npm").expect("a program"), readiness)
    .with_needs(needs.iter().map(|need| name(need)).collect())
    .expect("needs other services once each")
}

fn suite(layer: TestLayer, needs: &[&str]) -> TestSuite {
  let command = SuiteCommand::new(
    CommandProgram::new("cargo").expect("a program"),
    vec![CommandArg::new("test").expect("an argument")],
  )
  .expect("a command");
  let report = TestReport::new(ReportFormat::CargoJson, None).expect("a printed report");
  TestSuite::new(command, report, layer)
    .with_needs(SuiteNeeds::new(needs.iter().map(|need| Slug::new(need).expect("a slug")).collect()).expect("needs"))
    .with_timeout(CommandTimeout::from_secs(300).expect("in range"))
}

/// A project of two published services, one draft never published, a stack and two suites.
fn seed(settings: &Settings) {
  let mut db = settings.db();
  let now = Timestamp::now();
  let postgres = service("postgres", &[]).with_run_data(RunData::DatabaseContainer);
  let api = service("cloud-api", &["postgres"])
    .with_owned_paths(vec![OwnedPath::new("crates/cloud/**").expect("a path")])
    .expect("owned once");
  let web = service("cloud-web", &["cloud-api"]).with_ui_driver(UiDriver::Mcp);
  for (body, publish) in [(postgres, true), (api, true), (web, false)] {
    let id = create(&mut db, service_slug(body.name()), &body, now).expect("create a service");
    if publish {
      db.publish_draft::<Service>(id, now).expect("publish");
    }
  }
  let stack_name = StackName::new("cloud").expect("a name");
  let stack = Stack::new(stack_name.clone(), vec![name("cloud-api")]).expect("a stack");
  create(&mut db, stack_slug(&stack_name), &stack, now).expect("create a stack");
  for (slug, body) in [
    ("lint", suite(TestLayer::Lint, &[])),
    ("web-e2e", suite(TestLayer::E2e, &["cloud"])),
  ] {
    create(&mut db, Slug::new(slug).expect("a slug"), &body, now).expect("create a suite");
  }
}

#[test]
fn services_tab_without_services_offers_import_and_new() {
  let mut settings = Settings::new(false);
  settings.tab("services", "settings-services-empty");
  assert_eq!(settings.text("settings-services-empty-title"), "No services yet");
  assert_eq!(settings.text("settings-services-count"), "0");
  assert!(settings.exists("settings-services-import"));
  assert!(settings.exists("settings-services-new"));
  assert_eq!(settings.text("settings-stacks-empty-label"), "Nothing to group yet");
  assert!(!settings.exists("settings-stacks-new"), "a stack needs a service first");
  assert!(settings.exists("settings-suites-new"));
  assert!(!settings.exists("settings-flaky"), "Flaky tests waits for a suite");
  assert!(settings.exists("settings-services-agents"));

  settings.click("settings-services-import");
  settings.until("Import draft's page", |s| s.exists("settings-services-page-import"));
  assert_eq!(settings.text("settings-services-page-title"), "Import draft");
  assert_eq!(
    settings.text("settings-services-page-lead"),
    "This page is not built in this app yet."
  );
  settings.click("settings-services-page-crumb-back");
  settings.until("back on Services", |s| s.exists("settings-services-empty"));
}

#[test]
fn services_tab_lists_services_stacks_and_suites() {
  let mut settings = Settings::new(false);
  seed(&settings);
  settings.tab("services", "settings-service-postgres-row");
  assert_eq!(settings.text("settings-services-count"), "3");
  assert_eq!(settings.text("settings-service-postgres-driver-label"), "no UI");
  assert_eq!(
    settings.text("settings-service-postgres-hint"),
    "needs nothing · fresh database container per run"
  );
  assert_eq!(
    settings.text("settings-service-cloud-api-hint"),
    "needs postgres · owns crates/cloud/**"
  );
  assert_eq!(
    settings.text("settings-service-cloud-api-approval-label"),
    "Asks before each start"
  );
  assert_eq!(settings.text("settings-service-cloud-web-driver-label"), "mcp");
  assert_eq!(
    settings.text("settings-service-cloud-web-approval-label"),
    "Not published"
  );
  assert_eq!(settings.text("settings-stack-cloud-hint"), "postgres → cloud-api");
  assert_eq!(settings.text("settings-stack-cloud-slots"), "2 slots");
  assert_eq!(settings.text("settings-suite-lint-layer-label"), "lint");
  assert_eq!(
    settings.text("settings-suite-lint-hint"),
    "cargo json · no services · 5 min"
  );
  assert_eq!(
    settings.text("settings-suite-web-e2e-hint"),
    "cargo json · needs stack cloud · 5 min"
  );
  assert!(settings.exists("settings-flaky-quarantine-row"));
  assert!(!settings.exists("settings-services-agents"));

  settings.click("settings-stack-cloud-row");
  settings.until("the stack's page", |s| s.exists("settings-services-page-stack"));
  assert_eq!(settings.text("settings-services-page-title"), "Stack");
}

/// `navigate` opens the tab and its pages by name, a service's with its id, and answers their parameters back; an
/// item's page without its id, or a page of the tab under another, is refused.
#[test]
fn navigate_opens_the_services_pages_and_refuses_an_item_without_its_id() {
  let window = Window::open();
  let navigation = window.navigation();
  let project = project_id().to_string();
  let at = |extra: serde_json::Value| {
    let mut args = json!({ "route": "project_settings", "project": project, "tab": "services" });
    args
      .as_object_mut()
      .expect("an object")
      .extend(extra.as_object().expect("an object").clone());
    args
  };
  let answer = navigate(&navigation, &at(json!({}))).expect("the Services tab opens");
  assert_eq!(
    answer["route"]["path"],
    json!(format!("/project/{project}/settings?tab=services"))
  );
  let answer = navigate(&navigation, &at(json!({ "view": "import" }))).expect("Import draft opens");
  assert_eq!(
    answer["route"]["params"],
    json!({ "project": project, "tab": "services", "view": "import" })
  );
  let id = "019a2b3c-4d5e-7f60-8a9b-0c1d2e3f4a5b";
  let answer = navigate(&navigation, &at(json!({ "view": "service", "id": id }))).expect("a service's page opens");
  assert_eq!(
    answer["route"]["params"],
    json!({ "project": project, "tab": "services", "view": "service", "id": id })
  );
  assert!(navigate(&navigation, &at(json!({ "view": "service" }))).is_err());
  let runs = json!({ "route": "project_settings", "project": project, "tab": "runs", "view": "import" });
  assert!(navigate(&navigation, &runs).is_err());
}
