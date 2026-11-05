//! A service's page after Program and Needs (`02 · Service editor` `TGRTt`): Readiness, Seed, Per-run data, UI driver,
//! Owned paths and Approval each save as a field is left or a choice is made and read back when the page opens again;
//! what the core refuses is said in place and nothing is saved. Run with
//! `cargo test -p orchester-app --test project_settings service_sections`.

use orchester_core::{
  resource::HttpMethod,
  run::ProjectScope,
  service::{ReadyCheck, RunData, Seed, ServiceApproval, UiDriver},
  time::Timestamp,
};

use super::{
  harness::Settings,
  run_fixture::started_run,
  service_editor::{open_web, stored, type_and_leave},
};

/// Where leaving a field moves the focus: a field every service's page shows.
const ELSEWHERE: &str = "settings-service-program";

/// The page of `cloud-web` again, after going back to Services: what it shows is what was stored.
fn reopen(settings: &mut Settings) {
  settings.click("settings-service-editor-crumb-back");
  settings.until("Services", |s| s.exists("settings-service-cloud-web-row"));
  settings.click("settings-service-cloud-web-row");
  settings.until("the service's page", |s| s.exists(ELSEWHERE));
}

/// The error said under the field `id`.
fn error(settings: &mut Settings, id: &str) -> String {
  let note = format!("{id}-note");
  settings.until("the error", |s| s.exists(&note));
  settings.text(&format!("{note}-text"))
}

#[test]
fn service_sections_save_readiness_and_read_it_back() {
  let mut settings = open_web();
  settings.click("settings-service-ready-http");
  type_and_leave(&mut settings, ("settings-service-ready-path", "/healthz"), ELSEWHERE);
  type_and_leave(&mut settings, ("settings-service-timeout", "90"), ELSEWHERE);
  settings.saved();
  let web = stored(&settings, "cloud-web").expect("cloud-web");
  assert!(matches!(web.readiness().check(), ReadyCheck::Http(path) if path.as_str() == "/healthz"));
  assert_eq!(web.readiness().timeout().secs(), 90);
  reopen(&mut settings);
  assert_eq!(settings.value("settings-service-ready-path"), "/healthz");
  assert_eq!(settings.value("settings-service-timeout"), "90");
  assert_eq!(settings.text("settings-service-order-cloud-web-ready"), "HTTP /healthz");
}

#[test]
fn service_sections_say_readiness_errors_in_place() {
  let mut settings = open_web();
  settings.click("settings-service-ready-log");
  type_and_leave(&mut settings, ("settings-service-ready-pattern", "ready ("), ELSEWHERE);
  assert_eq!(
    error(&mut settings, "settings-service-ready-pattern"),
    "ready ( is not a regular expression."
  );
  type_and_leave(&mut settings, ("settings-service-timeout", "0"), ELSEWHERE);
  assert_eq!(
    error(&mut settings, "settings-service-timeout"),
    "Enter a whole number of seconds from 1 to 3600."
  );
  assert_eq!(
    settings.text("project-settings-save-status"),
    "Not saved · fix the marked field"
  );
  let web = stored(&settings, "cloud-web").expect("cloud-web");
  assert_eq!(*web.readiness().check(), ReadyCheck::PortOpen, "nothing was saved");
}

#[test]
fn service_sections_save_seed_requests_with_captures_in_order() {
  let mut settings = open_web();
  settings.click("settings-service-seed-requests");
  settings.until("the new request", |s| s.exists("settings-service-seed-request-0-url"));
  type_and_leave(
    &mut settings,
    ("settings-service-seed-request-0-url", "{{url}}/api/teams"),
    ELSEWHERE,
  );
  type_and_leave(
    &mut settings,
    ("settings-service-seed-request-0-body", r#"{"name": "QA team"}"#),
    ELSEWHERE,
  );
  settings.click("settings-service-seed-request-0-add-capture");
  settings.until("the capture", |s| {
    s.exists("settings-service-seed-request-0-capture-0-name")
  });
  settings.set("settings-service-seed-request-0-capture-0-name", "team_id");
  type_and_leave(
    &mut settings,
    ("settings-service-seed-request-0-capture-0-path", "$.id"),
    ELSEWHERE,
  );
  settings.saved();
  settings.click("settings-service-seed-add");
  settings.until("the second request", |s| {
    s.exists("settings-service-seed-request-1-url")
  });
  type_and_leave(
    &mut settings,
    (
      "settings-service-seed-request-1-url",
      "{{url}}/api/teams/{{seed.team_id}}",
    ),
    ELSEWHERE,
  );
  settings.saved();
  settings.click("settings-service-seed-request-1-up");
  settings.saved();
  let web = stored(&settings, "cloud-web").expect("cloud-web");
  let Seed::Requests(requests) = web.seed() else {
    panic!("a seed of requests: {:?}", web.seed());
  };
  let urls: Vec<&str> = requests.as_slice().iter().map(|request| request.url()).collect();
  assert_eq!(
    urls,
    ["{{url}}/api/teams/{{seed.team_id}}", "{{url}}/api/teams"],
    "moved up"
  );
  let first = &requests.as_slice()[1];
  assert_eq!(first.method(), HttpMethod::Post);
  assert_eq!(first.body(), Some(r#"{"name": "QA team"}"#));
  assert_eq!(first.captures()[0].name().as_str(), "team_id");
  assert_eq!(
    settings.text("settings-service-seed-request-0-error-text"),
    "Request 1 uses seed.team_id, which no request before it captures."
  );
  reopen(&mut settings);
  assert_eq!(
    settings.text("settings-service-seed-request-1-capture-0"),
    "team_id ← $.id"
  );
}

#[test]
fn service_sections_say_seed_errors_in_place_and_save_a_command() {
  let mut settings = open_web();
  settings.click("settings-service-seed-requests");
  settings.until("the new request", |s| s.exists("settings-service-seed-request-0-url"));
  type_and_leave(
    &mut settings,
    ("settings-service-seed-request-0-url", "teams"),
    ELSEWHERE,
  );
  settings.until("the request's error", |s| {
    s.exists("settings-service-seed-request-0-error")
  });
  assert_eq!(
    settings.text("settings-service-seed-request-0-error-text"),
    "Enter an http:// or https:// URL, or one that starts with a placeholder, without spaces."
  );
  assert!(matches!(
    stored(&settings, "cloud-web").expect("cloud-web").seed(),
    Seed::None
  ));

  settings.click("settings-service-seed-command");
  type_and_leave(&mut settings, ("settings-service-seed-program", "npm"), ELSEWHERE);
  type_and_leave(
    &mut settings,
    ("settings-service-seed-arguments", "run\nseed"),
    ELSEWHERE,
  );
  settings.saved();
  let web = stored(&settings, "cloud-web").expect("cloud-web");
  let Seed::Command { program, arguments } = web.seed() else {
    panic!("a seed command: {:?}", web.seed());
  };
  assert_eq!(program.as_str(), "npm");
  assert_eq!(arguments.len(), 2);
  reopen(&mut settings);
  assert_eq!(settings.value("settings-service-seed-program"), "npm");
}

#[test]
fn service_sections_save_per_run_data_and_say_its_errors() {
  let mut settings = open_web();
  settings.click("settings-service-data-folder");
  type_and_leave(&mut settings, ("settings-service-data-path", "../outside"), ELSEWHERE);
  assert_eq!(
    error(&mut settings, "settings-service-data-path"),
    "../outside is not a folder of the run's files: write a relative path without .., a drive or a leading slash."
  );
  type_and_leave(&mut settings, ("settings-service-data-path", "team-web"), ELSEWHERE);
  settings.saved();
  let web = stored(&settings, "cloud-web").expect("cloud-web");
  assert!(matches!(web.run_data(), RunData::RunFolder(folder) if folder.as_str() == "team-web"));
  assert_eq!(settings.text("settings-service-value-4-key"), "{{data}}");
  reopen(&mut settings);
  assert_eq!(settings.value("settings-service-data-path"), "team-web");

  settings.click("settings-service-data-container");
  settings.saved();
  assert_eq!(
    stored(&settings, "cloud-web").expect("cloud-web").run_data(),
    &RunData::DatabaseContainer
  );
}

#[test]
fn service_sections_save_the_ui_driver_and_where_it_opens() {
  let mut settings = open_web();
  assert!(
    !settings.exists("settings-service-opens-at"),
    "no driver, nowhere to open"
  );
  assert!(
    !settings.exists("settings-service-driver-browser"),
    "browser waits for the in-app Browser"
  );
  assert!(!settings.exists("settings-service-driver-cdp"));
  settings.click("settings-service-driver-mcp");
  settings.until("Opens at", |s| s.exists("settings-service-opens-at"));
  type_and_leave(
    &mut settings,
    ("settings-service-opens-at", "{{url}} /teams"),
    ELSEWHERE,
  );
  assert_eq!(
    error(&mut settings, "settings-service-opens-at"),
    "Write where ui_open starts without spaces, or leave it empty."
  );
  type_and_leave(&mut settings, ("settings-service-opens-at", "{{url}}/teams"), ELSEWHERE);
  settings.saved();
  let web = stored(&settings, "cloud-web").expect("cloud-web");
  assert_eq!(web.ui_driver(), UiDriver::Mcp);
  assert_eq!(web.ui_start().map(|start| start.as_str()), Some("{{url}}/teams"));
  assert_eq!(settings.text("settings-service-tool-2"), "ui_open cloud-web");
  reopen(&mut settings);
  assert_eq!(settings.value("settings-service-opens-at"), "{{url}}/teams");
}

#[test]
fn service_sections_add_and_remove_owned_paths() {
  let mut settings = open_web();
  settings.set("settings-service-owned-add", "../outside/**");
  settings.click("settings-service-owned-add-button");
  assert_eq!(
    error(&mut settings, "settings-service-owned-add"),
    "../outside/** is not a path in the repository: write a relative pattern without .., a drive or a leading slash, such as web/team/**."
  );
  settings.click("settings-service-owned-0-remove");
  settings.set("settings-service-owned-add", "web/team/**");
  settings.click("settings-service-owned-add-button");
  settings.saved();
  let web = stored(&settings, "cloud-web").expect("cloud-web");
  let owned: Vec<&str> = web.owned_paths().iter().map(|path| path.as_str()).collect();
  assert_eq!(owned, ["web/team/**"]);
  reopen(&mut settings);
  assert_eq!(settings.text("settings-service-owned-0-label"), "web/team/**");
  settings.click("settings-service-owned-0-remove");
  settings.saved();
  assert!(
    stored(&settings, "cloud-web")
      .expect("cloud-web")
      .owned_paths()
      .is_empty()
  );
}

#[test]
fn service_sections_say_whether_a_start_asks_for_approval() {
  let mut settings = open_web();
  assert_eq!(
    settings.text("settings-service-approval-state-label"),
    "Not approved yet"
  );
  let web = stored(&settings, "cloud-web").expect("cloud-web");
  let mut db = settings.db();
  let run = started_run(&mut db);
  db.add_project_approval_rule(
    &ProjectScope::Service(ServiceApproval::of(&web)),
    (run, "request:start"),
    Timestamp::now(),
  )
  .expect("a rule");
  reopen(&mut settings);
  assert_eq!(
    settings.text("settings-service-approval-state-label"),
    "Approved for every run"
  );
  type_and_leave(
    &mut settings,
    ("settings-service-arguments", "serve\n--quiet"),
    ELSEWHERE,
  );
  settings.until("asks again", |s| {
    s.text("settings-service-approval-state-label") == "Changed since approved"
  });
}

#[test]
fn service_sections_list_the_values_and_tools_of_a_run() {
  let mut settings = open_web();
  let keys: Vec<String> = (0..4)
    .map(|at| settings.text(&format!("settings-service-value-{at}-key")))
    .collect();
  assert_eq!(keys, ["{{port}}", "{{url}}", "{{services.cloud-api.url}}", "{{run}}"]);
  assert_eq!(settings.text("settings-service-value-2-words"), "cloud-api's URL");
  let tools: Vec<String> = (0..3)
    .map(|at| settings.text(&format!("settings-service-tool-{at}")))
    .collect();
  assert_eq!(
    tools,
    [
      "start_service cloud-web",
      "service_logs cloud-web",
      "stop_service cloud-web"
    ]
  );
}
