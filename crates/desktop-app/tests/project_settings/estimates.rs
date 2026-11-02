//! Runs › Estimates (the Runs window `g406O`): *Let Orchester estimate story points* is the project's own setting,
//! which only a person flips; a project nobody set it for keeps the application-wide setting it moved from.
use orchester_store::AppDb;

use super::harness::Settings;

const SWITCH: &str = "settings-estimate-points";

/// A person's presses on the switch until the project stores `wanted`: the switch takes none before its value is read.
fn press_until(page: &mut Settings, wanted: bool) {
  page.until("saved for the project", |p| {
    if p.db().estimates_story_points().expect("read") == Some(wanted) {
      return true;
    }
    p.click(SWITCH);
    false
  });
}

#[test]
fn project_settings_estimates_are_the_projects_and_only_a_person_turns_them_on() {
  let mut page = Settings::new(false);
  page.until("the estimates row", |p| p.exists(&format!("{SWITCH}-row")));
  assert!(page.text("settings-estimates").contains("Estimates"));
  assert!(
    page
      .text(&format!("{SWITCH}-hint"))
      .contains("counts toward the daily budget")
  );
  page.until("automation's change refused", |p| {
    lurq::app::events::with_synthetic_input(|| p.click(SWITCH));
    p.exists(&format!("{SWITCH}-blocked"))
  });
  assert_eq!(page.db().estimates_story_points().expect("read"), None);
  press_until(&mut page, true);
  assert!(!page.exists(&format!("{SWITCH}-blocked")));
  let app = AppDb::open(&page.window.data.database()).expect("the application database");
  assert!(
    !app.estimates_story_points().expect("read"),
    "the application-wide setting stays as it was"
  );
}

/// A project nobody set it for shows the application-wide value it moved from: on, so a person's press turns it off.
#[test]
fn project_settings_estimates_keep_the_application_setting_until_set_for_the_project() {
  let mut page = Settings::new(false);
  AppDb::open(&page.window.data.database())
    .expect("the application database")
    .set_estimates_story_points(true)
    .expect("on for every project before ORC-103");
  page.tab("general", "settings-project-name");
  page.tab("runs", &format!("{SWITCH}-row"));
  page.until("turned off for the project", |p| {
    match p.db().estimates_story_points().expect("read") {
      Some(false) => return true,
      Some(true) => panic!("the switch showed off: the application's setting was not the project's"),
      None => p.click(SWITCH),
    }
    false
  });
}

fn choose_decision(page: &mut Settings) {
  use orchester_core::{
    connection::{DecisionConnection, DecisionServerUrl},
    id::DecisionConnectionId,
    name::Name,
    resource::ModelId,
    run::review::{AutoModeSettings, ReviewerChoice},
  };
  let model = ModelId::new("decision-only").expect("model");
  let decision = DecisionConnection::new(
    DecisionConnectionId::generate(),
    Name::new("Decision reviewer").expect("name"),
    DecisionServerUrl::new("http://localhost:8000").expect("URL"),
    None,
  )
  .with_models(vec![model.clone()]);
  let mut db = page.db();
  db.create_decision_connection(&decision)
    .expect("create decision connection");
  db.save_auto_mode_settings(&AutoModeSettings {
    reviewer: Some(ReviewerChoice::decision(decision.id(), model, None)),
  })
  .expect("decision reviewer");
  page.tab("general", "settings-project-name");
  page.tab("runs", &format!("{SWITCH}-row"));
}

#[test]
fn estimates_decision_reviewer_row_names_the_default_language_connection() {
  let mut page = Settings::new(true);
  choose_decision(&mut page);
  page.until("language estimator hint", |p| {
    p.text(&format!("{SWITCH}-hint"))
      .contains("on Claude Code, as the reviewer writes no reasons")
  });
}

#[test]
fn estimates_decision_reviewer_row_says_unavailable_without_a_language_connection() {
  let mut page = Settings::new(false);
  choose_decision(&mut page);
  page.until("unavailable estimator reason", |p| {
    p.text(&format!("{SWITCH}-hint")).contains("Estimates unavailable")
  });
  assert!(page.text(&format!("{SWITCH}-hint")).contains("no language reviewer"));
  page.click(SWITCH);
  assert_eq!(page.db().estimates_story_points().expect("read"), None);
}

#[test]
fn estimates_decision_reviewer_failed_refresh_disables_until_successful_reread() {
  use orchester_core::store::ConnectionStore;
  use orchester_workspace::tasks::outside::Outside;

  let mut page = Settings::new(true);
  choose_decision(&mut page);
  press_until(&mut page, true);
  let connection = page.db().connections().expect("connections").remove(0);
  let sqlite = rusqlite::Connection::open(page.repo.join(".orchester/orchester.db")).expect("fixture DB");
  sqlite
    .execute("DELETE FROM connections", [])
    .expect("remove last language connection");
  // Keep the tab mounted with its cached settings, but make the next settings read fail.
  sqlite
    .execute("ALTER TABLE decision_reviewer_connection RENAME TO held_reviewer", [])
    .expect("reject read");
  page.outside(Outside::Connections);
  page.until("settings refresh error", |p| p.exists(&format!("{SWITCH}-error")));
  assert!(
    page
      .text(&format!("{SWITCH}-error"))
      .contains("This setting can't be read:")
  );
  assert!(!page.text(&format!("{SWITCH}-hint")).contains("on Claude Code"));
  page.click(SWITCH);
  assert_eq!(page.db().estimates_story_points().expect("unchanged"), Some(true));

  sqlite
    .execute("ALTER TABLE held_reviewer RENAME TO decision_reviewer_connection", [])
    .expect("allow read");
  page.outside(Outside::Connections);
  page.until("fresh unavailable state", |p| {
    p.text(&format!("{SWITCH}-hint")).contains("no language reviewer")
  });
  assert!(!page.exists(&format!("{SWITCH}-error")));
  page.click(SWITCH);
  assert_eq!(
    page.db().estimates_story_points().expect("still unavailable"),
    Some(true)
  );

  page
    .db()
    .create_connection(&connection)
    .expect("restore language connection");
  page.outside(Outside::Connections);
  page.until("fresh language estimator", |p| {
    p.text(&format!("{SWITCH}-hint")).contains("on Claude Code")
  });
  press_until(&mut page, false);
}
