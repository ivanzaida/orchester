//! A stack's page under Project settings › Services (`03 · Stack editor` `iNxTM`): it opens with every service a start
//! brings up in start order, the slots it takes and the tree of needs, saves its draft as a service is added or
//! removed, says the core's errors in place without saving, makes a new stack on its first save and publishes the
//! draft. Run with `cargo test -p orchester-app --test project_settings stack_editor`.

use orchester_core::{
  resource::{
    CommandArg, CommandProgram, ReportFormat, Slug, SuiteCommand, SuiteNeeds, TestLayer, TestReport, TestSuite,
  },
  service::{Readiness, ReadyCheck, ReadyTimeout, RunData, Service, ServiceName, Stack, StackName},
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

/// `postgres` (a database container), `cloud-api` and `team-server` that need it, `cloud-web` that needs `cloud-api`,
/// all published; the stack `full` of `cloud-web` and `team-server`, a draft; the suite `web-e2e` that needs it.
fn seed(settings: &Settings) {
  let mut db = settings.db();
  let now = Timestamp::now();
  for body in [
    service("postgres", &[]).with_run_data(RunData::DatabaseContainer),
    service("cloud-api", &["postgres"]),
    service("cloud-web", &["cloud-api"]),
    service("team-server", &["postgres"]),
  ] {
    let id = create(&mut db, service_slug(body.name()), &body, now).expect("create a service");
    db.publish_draft::<Service>(id, now).expect("publish");
  }
  let full = StackName::new("full").expect("a name");
  let stack = Stack::new(full.clone(), vec![name("cloud-web"), name("team-server")]).expect("a stack");
  create(&mut db, stack_slug(&full), &stack, now).expect("create a stack");
  let command = SuiteCommand::new(
    CommandProgram::new("npx").expect("a program"),
    vec![CommandArg::new("playwright").expect("an argument")],
  )
  .expect("a command");
  let report = TestReport::new(ReportFormat::PlaywrightJson, None).expect("a printed report");
  let suite = TestSuite::new(command, report, TestLayer::E2e)
    .with_needs(SuiteNeeds::new(vec![Slug::new("full").expect("a slug")]).expect("needs"));
  create(&mut db, Slug::new("web-e2e").expect("a slug"), &suite, now).expect("create a suite");
}

/// The page of `full`, opened from its row.
fn open_full() -> Settings {
  let mut settings = Settings::new(false);
  seed(&settings);
  settings.tab("services", "settings-stack-full-row");
  settings.click("settings-stack-full-row");
  settings.until("the stack's page", |s| s.exists("settings-stack-editor"));
  settings
}

/// The stack `value` as stored now: its draft if it has one, otherwise its latest version.
fn stored(settings: &Settings, value: &str) -> Option<Stack> {
  entries::<Stack, _>(&settings.db())
    .expect("stacks")
    .into_iter()
    .find(|entry| &*entry.name == value)
    .map(|entry| entry.body)
}

fn listed(stack: &Stack) -> Vec<&str> {
  stack.services().iter().map(ServiceName::as_str).collect()
}

#[test]
fn stack_editor_opens_with_its_services_in_start_order_its_slots_and_tree() {
  let mut settings = open_full();
  assert_eq!(settings.text("settings-stack-editor-crumb"), "Services / Stacks / full");
  assert_eq!(settings.text("settings-stack-editor-head-title"), "full");
  assert_eq!(settings.value("settings-stack-name"), "full");
  assert_eq!(settings.text("settings-stack-services-count"), "4");
  for (service, step, hint) in [
    ("postgres", "1", "Needed by cloud-api, team-server"),
    ("cloud-api", "2", "Needed by cloud-web"),
    ("team-server", "2", "Chosen · starts with cloud-api"),
    ("cloud-web", "3", "Chosen"),
  ] {
    let id = format!("settings-stack-service-{service}");
    assert_eq!(settings.text(&format!("{id}-step")), step, "{service}'s step");
    assert_eq!(settings.text(&format!("{id}-hint")), hint, "{service}'s hint");
  }
  assert!(settings.exists("settings-stack-service-postgres-from-needs"));
  assert!(settings.exists("settings-stack-service-cloud-web-remove"));
  assert!(!settings.exists("settings-stack-service-cloud-api-remove"));
  assert_eq!(settings.text("settings-stack-service-postgres-driver-label"), "no UI");
  settings.until("the computer's slots", |s| s.exists("settings-stack-slots-bar"));
  assert_eq!(settings.text("settings-stack-slots-label"), "Takes 4 of 8 slots");
  assert!(settings.exists("settings-stack-slot-7"));
  assert!(!settings.exists("settings-stack-slot-8"));
  assert_eq!(
    settings.text("settings-stack-dependencies"),
    "Dependencies postgres cloud-api cloud-web team-server Each arrow is a need. A crash stops what depends on it."
  );
  assert!(settings.exists("settings-stack-used-by-web-e2e-row"));
  assert_eq!(settings.text("settings-stack-state"), "Draft · not published yet");
  assert_eq!(settings.text("project-settings-save-status"), "All changes saved");
}

#[test]
fn stack_editor_saves_the_draft_as_a_service_is_added_or_removed() {
  let mut settings = open_full();
  settings.click("settings-stack-service-team-server-remove");
  settings.until("team-server taken out", |s| {
    stored(s, "full").is_some_and(|full| listed(&full) == ["cloud-web"])
  });
  settings.saved();
  assert!(!settings.exists("settings-stack-service-team-server-row"));

  settings.choose("settings-stack-add-service", "team-server");
  settings.until("team-server added", |s| {
    stored(s, "full").is_some_and(|full| listed(&full) == ["cloud-web", "team-server"])
  });
  settings.saved();
  assert!(settings.exists("settings-stack-service-team-server-row"));
}

#[test]
fn stack_editor_says_the_cores_errors_in_place_and_saves_nothing() {
  let mut settings = open_full();
  settings.click("settings-stack-service-team-server-remove");
  settings.until("team-server taken out", |s| {
    stored(s, "full").is_some_and(|full| listed(&full) == ["cloud-web"])
  });
  settings.saved();
  settings.click("settings-stack-service-cloud-web-remove");
  settings.until("the stack's error", |s| s.exists("settings-stack-services-error"));
  assert_eq!(
    settings.text("settings-stack-services-error-text"),
    "Add a service: a stack starts at least one."
  );
  assert_eq!(
    settings.text("project-settings-save-status"),
    "Not saved · fix the marked field"
  );
  assert_eq!(
    listed(&stored(&settings, "full").expect("full")),
    ["cloud-web"],
    "nothing was saved"
  );
}

#[test]
fn new_stack_is_made_on_its_first_save_and_opens_under_its_id() {
  let mut settings = Settings::new(false);
  seed(&settings);
  settings.tab("services", "settings-stacks-new");
  settings.click("settings-stacks-new");
  settings.until("New stack's page", |s| s.exists("settings-stack-name"));
  assert_eq!(settings.text("settings-stack-state"), "Not saved yet");
  type_and_leave(
    &mut settings,
    ("settings-stack-name", "Team Stack"),
    "settings-stack-add-service",
  );
  settings.until("the name's error", |s| s.exists("settings-stack-name-note"));
  assert_eq!(
    settings.text("settings-stack-name-note-text"),
    "Team Stack is not a stack name: 1 to 64 lowercase letters, digits and single hyphens, such as full."
  );
  type_and_leave(
    &mut settings,
    ("settings-stack-name", "team"),
    "settings-stack-add-service",
  );
  settings.choose("settings-stack-add-service", "team-server");
  settings.until("the new stack's page", |s| {
    s.exists("settings-stack-editor") && s.text("settings-stack-state") == "Draft · not published yet"
  });
  settings.saved();
  let team = stored(&settings, "team").expect("team is made");
  assert_eq!(listed(&team), ["team-server"]);
  assert_eq!(settings.text("settings-stack-editor-head-title"), "team");
}

#[test]
fn stack_editor_publishes_the_draft() {
  let mut settings = open_full();
  settings.click("settings-stack-publish");
  settings.until("published", |s| s.text("settings-stack-state") == "Published v1");
  let db = settings.db();
  let id = entries::<Stack, _>(&db)
    .expect("stacks")
    .into_iter()
    .find(|entry| &*entry.name == "full")
    .expect("full")
    .id;
  assert!(db.latest_version::<Stack>(id).expect("versions").is_some());
  assert!(db.draft::<Stack>(id).expect("draft").is_none());
  assert!(!settings.exists("settings-stack-publish"));
}
