//! Project settings › Services › Flaky tests (`09 · Flaky tests` `Tv9VC`, ORC-267): the quarantined tests and the
//! flaky ones not quarantined from the project's real flaky history, each with how often it was flaky and when it was
//! last seen; *Quarantine now* and *Unquarantine* move a test through the workspace and the move stays when the page
//! is opened again; an MCP client's *Quarantine now* waits for the person's switch. Run with
//! `cargo test -p orchester-app --test project_settings flaky_tests`.

use lurq::app::events::with_synthetic_input;
use orchester_app::routes::{ProjectPage, Route, ServicesPage};
use orchester_core::{
  resource::Slug,
  test_step::flaky::{FlakyKind, FlakyOutcome, TestKey},
  time::Timestamp,
};

use super::{harness::Settings, run_fixture::run_ids, service_editor::automation_changes_permissions};
use crate::support::project_id;

fn key(suite: &str, test: &str) -> TestKey {
  TestKey::new(Slug::new(suite).expect("a slug"), test).expect("a test")
}

fn billing() -> TestKey {
  key("web-e2e", "billing › shows the invoice list")
}

fn teams() -> TestKey {
  key("web-e2e", "teams › renames a team")
}

fn refresh() -> TestKey {
  key("rust-unit", "auth::tests::refresh_expires")
}

/// A history of four runs: billing and teams flaky in three of them, refresh in one; billing quarantined.
fn seed(settings: &Settings) {
  let mut db = settings.db();
  let runs = run_ids(&mut db, 4);
  let now = Timestamp::now();
  let mut outcomes = Vec::new();
  for run in &runs[..3] {
    for test in [billing(), teams()] {
      outcomes.push(FlakyOutcome {
        test,
        kind: FlakyKind::Flaky,
        run: *run,
        at: now,
      });
    }
  }
  outcomes.push(FlakyOutcome {
    test: refresh(),
    kind: FlakyKind::Flaky,
    run: runs[3],
    at: now,
  });
  db.record_flaky_outcomes(&outcomes).expect("the runner keeps them");
  assert!(db.quarantine_test(&billing(), now).expect("quarantine"));
}

/// Flaky tests opened on the seeded history.
fn opened() -> Settings {
  let mut settings = Settings::new(false);
  seed(&settings);
  open(&mut settings);
  settings
}

fn open(settings: &mut Settings) {
  settings
    .window
    .navigation()
    .go(Route::Project(project_id(), ProjectPage::Services(ServicesPage::Flaky)));
  settings.until("the flaky tests", |s| s.exists("settings-flaky-quarantined-rows"));
}

fn quarantined(settings: &Settings) -> Vec<TestKey> {
  settings
    .db()
    .quarantined_tests()
    .expect("read")
    .into_iter()
    .map(|(test, _)| test)
    .collect()
}

#[test]
fn flaky_tests_shows_quarantined_and_watched_tests_from_the_history() {
  let mut settings = opened();
  assert_eq!(settings.text("settings-flaky-crumb-here"), "Flaky tests");
  assert_eq!(settings.text("settings-flaky-head-title"), "Flaky tests");
  assert!(
    settings
      .text("settings-flaky-head-lead")
      .contains("After 3 flaky runs in 14 days it is watched here")
  );
  assert_eq!(settings.text("settings-flaky-quarantined-count"), "1");
  assert_eq!(
    settings.text("settings-flaky-quarantined-0-label"),
    "billing › shows the invoice list"
  );
  assert_eq!(
    settings.text("settings-flaky-quarantined-0-hint"),
    "web-e2e · last seen just now"
  );
  assert_eq!(settings.text("settings-flaky-quarantined-0-rate"), "flaky in 3 runs");
  assert!(
    settings
      .text("settings-flaky-quarantined-0-stands")
      .starts_with("quarantined ")
  );
  assert!(settings.exists("settings-flaky-quarantined-0-release"));

  assert_eq!(settings.text("settings-flaky-watching-count"), "2");
  assert_eq!(settings.text("settings-flaky-watching-note"), "Flaky, not quarantined");
  assert_eq!(
    settings.text("settings-flaky-watching-0-label"),
    "teams › renames a team"
  );
  assert_eq!(settings.text("settings-flaky-watching-0-rate"), "flaky in 3 runs");
  assert_eq!(
    settings.text("settings-flaky-watching-0-stands"),
    "watched, not quarantined"
  );
  assert_eq!(
    settings.text("settings-flaky-watching-1-label"),
    "auth::tests::refresh_expires"
  );
  assert_eq!(
    settings.text("settings-flaky-watching-1-hint"),
    "rust-unit · last seen just now"
  );
  assert_eq!(settings.text("settings-flaky-watching-1-rate"), "flaky in 1 run");
  assert_eq!(settings.text("settings-flaky-watching-1-stands"), "not quarantined");
  assert!(settings.exists("settings-flaky-watching-1-quarantine"));

  assert_eq!(settings.text("settings-flaky-rule-after-label"), "Watched after");
  assert_eq!(settings.value("settings-flaky-rule-runs"), "3");
  assert_eq!(settings.value("settings-flaky-rule-days"), "14");
}

#[test]
fn flaky_tests_quarantine_and_unquarantine_act_and_persist() {
  let mut settings = opened();
  settings.click("settings-flaky-watching-0-quarantine");
  settings.until("teams quarantined", |s| {
    s.text("settings-flaky-quarantined-count") == "2"
  });
  assert_eq!(quarantined(&settings), [billing(), teams()]);
  assert_eq!(
    settings.text("settings-flaky-quarantined-1-label"),
    "teams › renames a team"
  );
  assert_eq!(settings.text("settings-flaky-watching-count"), "1");

  settings.click("settings-flaky-quarantined-0-release");
  settings.until("billing released", |s| {
    s.text("settings-flaky-quarantined-count") == "1"
  });
  assert_eq!(quarantined(&settings), [teams()]);
  assert_eq!(
    settings.text("settings-flaky-watching-0-label"),
    "billing › shows the invoice list"
  );

  // Opened again from Services, the page reads what was kept.
  settings.click("settings-flaky-crumb-back");
  settings.until("back on Services", |s| !s.exists("settings-flaky"));
  open(&mut settings);
  assert_eq!(settings.text("settings-flaky-quarantined-count"), "1");
  assert_eq!(
    settings.text("settings-flaky-quarantined-0-label"),
    "teams › renames a team"
  );
  assert_eq!(settings.text("settings-flaky-watching-count"), "2");
}

/// An MCP client's *Quarantine now* quarantines nothing while *Let automation change agent permissions* is off, and
/// says so under the row; its *Unquarantine* is taken.
#[test]
fn flaky_tests_quarantine_by_an_mcp_client_waits_for_the_automation_setting() {
  let mut settings = opened();
  automation_changes_permissions(&settings, false);
  with_synthetic_input(|| settings.click("settings-flaky-watching-0-quarantine"));
  settings.until("the refusal", |s| s.exists("settings-flaky-watching-0-person-only"));
  for _ in 0..10 {
    settings.window.tree.tick_futures();
    settings.window.pass();
  }
  assert_eq!(quarantined(&settings), [billing()], "nothing was quarantined");

  with_synthetic_input(|| settings.click("settings-flaky-quarantined-0-release"));
  settings.until("billing released", |s| {
    s.text("settings-flaky-quarantined-count") == "0"
  });
  assert!(quarantined(&settings).is_empty());
  assert!(settings.exists("settings-flaky-quarantined-empty-row"));

  automation_changes_permissions(&settings, true);
  with_synthetic_input(|| settings.click("settings-flaky-watching-0-quarantine"));
  settings.until("quarantined under the setting", |s| {
    s.text("settings-flaky-quarantined-count") == "1"
  });
  assert!(!settings.exists("settings-flaky-watching-0-person-only"));
}
