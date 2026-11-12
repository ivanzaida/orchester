//! Runs › Estimates (the Runs window `g406O`): *Let Orchester estimate story points* is the project's own setting,
//! which only a person flips; the application-wide setting it moved from is no longer read (ORC-595).
use orchester_store::AppDb;

use super::harness::Settings;

const SWITCH: &str = "settings-estimate-points";

/// A person's presses on the switch until the project stores `wanted`: the switch takes none before its value is read.
fn press_until(page: &mut Settings, wanted: bool) {
  page.until("saved for the project", |p| {
    if p.db().estimates_story_points().expect("read") == wanted {
      return true;
    }
    p.click(SWITCH);
    false
  });
}

#[test]
fn project_settings_estimates_are_the_projects_and_only_a_person_turns_them_on() {
  let mut page = Settings::new(false);
  // The switch is a bone until the project's value is read; wait for the switch itself.
  page.until("the estimates switch", |p| p.exists(SWITCH));
  assert!(page.text("settings-estimates").contains("Estimates"));
  assert!(
    page
      .text(&format!("{SWITCH}-hint"))
      .contains("counts toward the daily budget")
  );
  page.until("automation's change refused to it", |p| {
    lurq::app::events::with_synthetic_input(|| p.click(SWITCH));
    p.window.refused_press().is_some()
  });
  assert!(!page.exists(&format!("{SWITCH}-blocked")), "the row says nothing of it");
  assert!(!page.switch_on(SWITCH), "the switch stays off");
  assert!(!page.db().estimates_story_points().expect("read"));
  press_until(&mut page, true);
  assert!(page.window.refused_press().is_none());
  let app = AppDb::open(&page.window.data.database()).expect("the application database");
  assert!(
    !app.estimates_story_points().expect("read"),
    "the application-wide setting stays as it was"
  );
}

/// The row shows the project's own value, whatever the application-wide setting it moved from says; a change counted
/// for the project (as the workspace's writer counts one) reads it again.
#[test]
fn project_settings_estimates_show_the_projects_value_not_the_application_setting() {
  let mut page = Settings::new(false);
  page.db().set_estimates_story_points(true).expect("on for the project");
  page.outside(orchester_workspace::tasks::outside::Outside::Estimates);
  page.tab("general", "settings-project-name");
  page.tab("runs", &format!("{SWITCH}-mark"));
  page.until("the project's value shown", |p| p.switch_on(SWITCH));
  assert!(
    !AppDb::open(&page.window.data.database())
      .expect("the application database")
      .estimates_story_points()
      .expect("read"),
    "the application-wide setting is off"
  );
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
  // Counted as the workspace's writer counts a reviewer's change, which reads the settings again.
  page.outside(orchester_workspace::tasks::outside::Outside::AutoMode);
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
  // A project without the setting reads off; the switch saved nothing.
  assert!(!page.db().estimates_story_points().expect("read"));
}

#[test]
fn estimates_decision_reviewer_failed_refresh_disables_until_successful_reread() {
  use orchester_core::store::ConnectionStore;
  use orchester_workspace::tasks::outside::Outside;

  let mut page = Settings::new(true);
  choose_decision(&mut page);
  // The switch is a bone until the project's value is read.
  page.until("the estimates switch", |p| p.exists(SWITCH));
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
  assert!(page.db().estimates_story_points().expect("unchanged"));

  sqlite
    .execute("ALTER TABLE held_reviewer RENAME TO decision_reviewer_connection", [])
    .expect("allow read");
  page.outside(Outside::Connections);
  page.until("fresh unavailable state", |p| {
    p.text(&format!("{SWITCH}-hint")).contains("no language reviewer")
  });
  assert!(!page.exists(&format!("{SWITCH}-error")));
  page.click(SWITCH);
  assert!(page.db().estimates_story_points().expect("still unavailable"));

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
