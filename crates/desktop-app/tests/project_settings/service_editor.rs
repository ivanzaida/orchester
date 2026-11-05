//! A service's page under Project settings › Services (`02 · Service editor` `TGRTt`): it opens a draft with its
//! fields, saves the draft as a field is left, says the core's errors in place without saving, makes a new service on
//! its first save, publishes the draft and deletes the service after its modal. Run with
//! `cargo test -p orchester-app --test project_settings service_editor`.

use lurq::app::events::with_synthetic_input;
use orchester_core::{
  resource::{CommandArg, CommandProgram},
  service::{Readiness, ReadyCheck, ReadyTimeout, Service, ServiceName, ServiceValue},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_workspace::agent_testing::data::{create, entries, service_slug};

use super::harness::Settings;

fn name(value: &str) -> ServiceName {
  ServiceName::new(value).expect("a kebab-case name")
}

/// This test program, which a start finds at its absolute path on every machine.
pub(super) fn program() -> String {
  std::env::current_exe()
    .expect("the test program")
    .to_str()
    .expect("a path in text")
    .to_owned()
}

fn service(value: &str, needs: &[&str]) -> Service {
  let readiness = Readiness::new(ReadyCheck::PortOpen, ReadyTimeout::from_secs(30).expect("in range"));
  Service::new(
    name(value),
    CommandProgram::new(&program()).expect("a program"),
    readiness,
  )
  .with_arguments(vec![CommandArg::new("serve").expect("an argument")])
  .with_needs(needs.iter().map(|need| name(need)).collect())
  .expect("needs other services once each")
}

/// `postgres` and `cloud-api` published, `cloud-web` a draft that needs `cloud-api`.
fn seed(settings: &Settings) {
  let mut db = settings.db();
  let now = Timestamp::now();
  for (body, publish) in [
    (service("postgres", &[]), true),
    (service("cloud-api", &["postgres"]), true),
    (service("cloud-web", &["cloud-api"]), false),
  ] {
    let id = create(&mut db, service_slug(body.name()), &body, now).expect("create a service");
    if publish {
      db.publish_draft::<Service>(id, now).expect("publish");
    }
  }
}

/// The page of `cloud-web`, opened from its row.
pub(super) fn open_web() -> Settings {
  let mut settings = Settings::new(false);
  seed(&settings);
  settings.tab("services", "settings-service-cloud-web-row");
  settings.click("settings-service-cloud-web-row");
  settings.until("the service's page", |s| s.exists("settings-service-program"));
  settings
}

/// The service `value` as stored now: its draft if it has one, otherwise its latest version.
pub(super) fn stored(settings: &Settings, value: &str) -> Option<Service> {
  entries::<Service, _>(&settings.db())
    .expect("services")
    .into_iter()
    .find(|entry| &*entry.name == value)
    .map(|entry| entry.body)
}

/// Types `text` into the field `id` and leaves it for `next`, as clicking into the next field does.
pub(super) fn type_and_leave(settings: &mut Settings, (id, text): (&str, &str), next: &str) {
  settings.set(id, text);
  settings
    .window
    .tree
    .get_element_by_id_mut(next)
    .unwrap_or_else(|| panic!("{next} is shown"))
    .focus();
  settings.window.pass();
}

#[test]
fn service_editor_opens_a_draft_with_its_fields_and_start_order() {
  let mut settings = open_web();
  assert_eq!(settings.text("settings-service-editor-crumb-here"), "cloud-web");
  assert_eq!(settings.text("settings-service-editor-head-title"), "cloud-web");
  assert_eq!(settings.value("settings-service-program"), program());
  assert_eq!(settings.value("settings-service-arguments"), "serve");
  assert_eq!(settings.value("settings-service-folder"), "");
  settings.until("the program's place", |s| s.exists("settings-service-program-found"));
  assert_eq!(settings.text("settings-service-program-found"), "Found at this path");
  assert!(settings.exists("settings-service-need-cloud-api"));
  assert_eq!(
    settings.text("settings-service-chain"),
    "1 postgres 2 cloud-api 3 cloud-web"
  );
  assert_eq!(settings.text("settings-service-order-postgres-ready"), "port");
  assert_eq!(settings.text("settings-service-start-order-title"), "Start order");
  assert_eq!(settings.text("settings-service-state"), "Draft · not published yet");
  assert!(!settings.exists("settings-service-name"), "a service keeps its name");
  assert_eq!(settings.text("project-settings-save-status"), "All changes saved");
}

#[test]
fn service_editor_saves_the_draft_as_a_field_is_left() {
  let mut settings = open_web();
  type_and_leave(
    &mut settings,
    ("settings-service-arguments", "serve\n--port\n{{port}}"),
    "settings-service-folder",
  );
  settings.saved();
  let web = stored(&settings, "cloud-web").expect("cloud-web");
  let arguments: Vec<&str> = web.arguments().iter().map(CommandArg::as_str).collect();
  assert_eq!(arguments, ["serve", "--port", "{{port}}"]);

  settings.click("settings-service-add-secret");
  settings.until("the secret's row", |s| s.exists("settings-service-variable-0-name"));
  settings.set("settings-service-variable-0-name", "SESSION_SECRET");
  type_and_leave(
    &mut settings,
    ("settings-service-variable-0-value", "TEAM_SESSION_SECRET"),
    "settings-service-folder",
  );
  settings.saved();
  let web = stored(&settings, "cloud-web").expect("cloud-web");
  let variables: Vec<(&str, &str)> = web
    .variables()
    .iter()
    .map(|(name, value)| match value {
      ServiceValue::Secret(secret) => (name.as_str(), secret.as_str()),
      ServiceValue::Plain(_) => panic!("{name} takes a secret"),
    })
    .collect();
  assert_eq!(variables, [("SESSION_SECRET", "TEAM_SESSION_SECRET")]);
}

#[test]
fn service_editor_says_the_cores_errors_in_place_and_saves_nothing() {
  let mut settings = open_web();
  type_and_leave(
    &mut settings,
    ("settings-service-folder", "../outside"),
    "settings-service-arguments",
  );
  settings.until("the folder's error", |s| s.exists("settings-service-folder-note"));
  assert_eq!(
    settings.text("settings-service-folder-note-text"),
    "../outside is not a folder in the run's worktree: write a relative path without .., a drive or a leading slash."
  );
  assert_eq!(
    settings.text("project-settings-save-status"),
    "Not saved · fix the marked field"
  );
  let web = stored(&settings, "cloud-web").expect("cloud-web");
  assert!(web.working_folder().is_root(), "nothing was saved");

  type_and_leave(
    &mut settings,
    ("settings-service-folder", ""),
    "settings-service-arguments",
  );
  type_and_leave(
    &mut settings,
    ("settings-service-arguments", "{{services.postgres.url}}"),
    "settings-service-folder",
  );
  settings.until("the placeholder's error", |s| {
    s.exists("settings-service-arguments-note")
  });
  assert_eq!(
    settings.text("settings-service-arguments-note-text"),
    "Argument 1 uses postgres, which this service does not need. Add it under Needs."
  );
  assert!(settings.exists("settings-service-publish"), "Publish says it waits");
}

#[test]
fn new_service_is_made_on_its_first_save_and_opens_under_its_id() {
  let mut settings = Settings::new(false);
  seed(&settings);
  settings.tab("services", "settings-services-new");
  settings.click("settings-services-new");
  settings.until("New service's page", |s| s.exists("settings-service-name"));
  assert_eq!(settings.text("settings-service-state"), "Not saved yet");
  settings.set("settings-service-name", "team-web");
  type_and_leave(
    &mut settings,
    ("settings-service-program", &program()),
    "settings-service-arguments",
  );
  settings.until("the new service's page", |s| {
    s.exists("settings-service-program") && !s.exists("settings-service-name")
  });
  settings.saved();
  let web = stored(&settings, "team-web").expect("team-web is made");
  assert_eq!(web.program().as_str(), program());
  assert_eq!(settings.text("settings-service-editor-crumb-here"), "team-web");
  assert_eq!(settings.text("settings-service-state"), "Draft · not published yet");
}

#[test]
fn service_editor_publishes_the_draft() {
  let mut settings = open_web();
  settings.click("settings-service-publish");
  settings.until("published", |s| s.text("settings-service-state") == "Published v1");
  let db = settings.db();
  let id = entries::<Service, _>(&db)
    .expect("services")
    .into_iter()
    .find(|entry| &*entry.name == "cloud-web")
    .expect("cloud-web")
    .id;
  assert!(db.latest_version::<Service>(id).expect("versions").is_some());
  assert!(db.draft::<Service>(id).expect("draft").is_none());
  assert!(!settings.exists("settings-service-publish"));
}

#[test]
fn service_editor_deletes_the_service_after_its_modal() {
  let mut settings = open_web();
  settings.click("settings-service-delete");
  settings.until("the delete question", |s| s.exists("settings-service-delete-dialog"));
  assert_eq!(
    settings.text("settings-service-delete-dialog-title"),
    "Delete cloud-web?"
  );
  settings.click("settings-service-delete-dialog-cancel");
  settings.until("the question closed", |s| !s.exists("settings-service-delete-dialog"));
  assert!(stored(&settings, "cloud-web").is_some(), "Cancel keeps it");

  settings.click("settings-service-delete");
  settings.until("the delete question", |s| s.exists("settings-service-delete-confirm"));
  settings.click("settings-service-delete-confirm");
  settings.until("back on Services", |s| s.exists("settings-service-postgres-row"));
  assert!(stored(&settings, "cloud-web").is_none(), "cloud-web is deleted");
  assert!(!settings.exists("settings-service-cloud-web-row"));
}

/// A change made while a save is under way saves once it lands: a need removed right after a field was left.
#[test]
fn service_editor_saves_a_change_made_while_a_save_is_under_way() {
  let mut settings = open_web();
  type_and_leave(
    &mut settings,
    ("settings-service-arguments", "serve\n--quiet"),
    "settings-service-folder",
  );
  settings.click("settings-service-need-cloud-api-remove");
  settings.until("both changes stored", |s| {
    stored(s, "cloud-web").is_some_and(|web| web.needs().is_empty() && web.arguments().len() == 2)
  });
  settings.saved();
}

/// Lets an MCP client's window input change what a service runs, or not.
fn automation_changes_permissions(settings: &Settings, on: bool) {
  settings
    .window
    .launch
    .workspace()
    .expect("the session started")
    .set_automation_changes_permissions(on)
    .expect("the permissions opt-in");
}

/// An MCP client's program is put back and not saved while *Let automation change agent permissions* is off, even
/// when a person's save of another field follows; the person's own change saves.
#[test]
fn service_editor_puts_back_what_an_mcp_client_typed_into_the_program() {
  let mut settings = open_web();
  with_synthetic_input(|| {
    type_and_leave(
      &mut settings,
      ("settings-service-program", "other-tool"),
      "settings-service-arguments",
    );
  });
  settings.until("the refusal", |s| s.exists("settings-service-person-only"));
  assert_eq!(settings.value("settings-service-program"), program(), "put back");
  assert_eq!(
    stored(&settings, "cloud-web").expect("cloud-web").program().as_str(),
    program()
  );

  // Typed but not left: a person's save of another field puts it back too.
  with_synthetic_input(|| settings.set("settings-service-program", "other-tool"));
  type_and_leave(
    &mut settings,
    ("settings-service-arguments", "serve\n--quiet"),
    "settings-service-folder",
  );
  settings.until("the person's arguments stored", |s| {
    stored(s, "cloud-web").is_some_and(|web| web.arguments().len() == 2)
  });
  settings.saved();
  assert_eq!(
    stored(&settings, "cloud-web").expect("cloud-web").program().as_str(),
    program(),
    "the client's program is not saved with the person's change"
  );
  assert_eq!(settings.value("settings-service-program"), program());
}

/// Under *Let automation change agent permissions* an MCP client's program saves.
#[test]
fn service_editor_takes_an_mcp_clients_program_under_the_automation_setting() {
  let mut settings = open_web();
  automation_changes_permissions(&settings, true);
  with_synthetic_input(|| {
    type_and_leave(
      &mut settings,
      ("settings-service-program", "other-tool"),
      "settings-service-arguments",
    );
  });
  settings.until("the client's program stored", |s| {
    stored(s, "cloud-web").is_some_and(|web| web.program().as_str() == "other-tool")
  });
  settings.saved();
  assert!(!settings.exists("settings-service-person-only"));
}

/// An MCP client's Publish is refused while the setting is off and taken while it is on.
#[test]
fn service_editor_publishes_for_an_mcp_client_only_under_the_automation_setting() {
  let mut settings = open_web();
  with_synthetic_input(|| settings.click("settings-service-publish"));
  settings.until("the refusal", |s| s.exists("settings-service-person-only"));
  assert_eq!(settings.text("settings-service-state"), "Draft · not published yet");

  automation_changes_permissions(&settings, true);
  with_synthetic_input(|| settings.click("settings-service-publish"));
  settings.until("published", |s| s.text("settings-service-state") == "Published v1");
  assert!(!settings.exists("settings-service-person-only"));
}

/// Only a person deletes a service: an MCP client's confirm is refused in the modal, with the setting on too.
#[test]
fn service_editor_refuses_an_mcp_clients_delete() {
  let mut settings = open_web();
  automation_changes_permissions(&settings, true);
  settings.click("settings-service-delete");
  settings.until("the delete question", |s| s.exists("settings-service-delete-confirm"));
  with_synthetic_input(|| settings.click("settings-service-delete-confirm"));
  settings.until("the refusal", |s| {
    s.exists("settings-service-delete-dialog-person-only")
  });
  for _ in 0..10 {
    settings.window.tree.tick_futures();
    settings.window.pass();
  }
  assert!(stored(&settings, "cloud-web").is_some(), "not deleted");
}
