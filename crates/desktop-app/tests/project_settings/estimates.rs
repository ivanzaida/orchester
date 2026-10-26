//! Runs › Estimates (the Runs window `g406O`): *Let Orchester estimate story points* is the project's own setting,
//! which only a person flips; a project nobody set it for keeps the application-wide setting it moved from.
use orchester_store::AppDb;

use super::harness::Settings;

const SWITCH: &str = "settings-estimate-points";

/// Waits until the switch can be flipped: the stored value is read and the row lived through a pass.
fn armed(page: &mut Settings) {
  page.until("the estimates row", |p| p.exists(&format!("{SWITCH}-row")));
  for _ in 0..5 {
    page.window.tree.tick_futures();
    page.window.pass();
  }
}

#[test]
fn project_settings_estimates_are_the_projects_and_only_a_person_turns_them_on() {
  let mut page = Settings::new(false);
  armed(&mut page);
  assert!(page.text("settings-estimates").contains("Estimates"));
  assert!(
    page
      .text(&format!("{SWITCH}-hint"))
      .contains("counts toward the daily budget")
  );
  lurq::app::events::with_synthetic_input(|| page.click(SWITCH));
  assert!(
    page.exists(&format!("{SWITCH}-blocked")),
    "automation's change is not taken"
  );
  assert_eq!(page.db().estimates_story_points().expect("read"), None);
  page.click(SWITCH);
  page.until("saved for the project", |p| {
    p.db().estimates_story_points().expect("read") == Some(true)
  });
  assert!(!page.exists(&format!("{SWITCH}-blocked")));
  let app = AppDb::open(&page.window.data.database()).expect("the application database");
  assert!(
    !app.estimates_story_points().expect("read"),
    "the application-wide setting stays as it was"
  );
}

/// A project nobody set it for shows the application-wide value it moved from: on, a person's press turns it off.
#[test]
fn project_settings_estimates_keep_the_application_setting_until_set_for_the_project() {
  let mut page = Settings::new(false);
  AppDb::open(&page.window.data.database())
    .expect("the application database")
    .set_estimates_story_points(true)
    .expect("on for every project before ORC-103");
  page.tab("general", "settings-project-name");
  page.tab("runs", &format!("{SWITCH}-row"));
  armed(&mut page);
  page.click(SWITCH);
  page.until("turned off for the project", |p| {
    p.db().estimates_story_points().expect("read") == Some(false)
  });
}
