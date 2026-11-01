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
  assert!(!page.db().estimates_story_points().expect("read"));
  press_until(&mut page, true);
  assert!(!page.exists(&format!("{SWITCH}-blocked")));
  let app = AppDb::open(&page.window.data.database()).expect("the application database");
  assert!(
    !app.estimates_story_points().expect("read"),
    "the application-wide setting stays as it was"
  );
}

/// The row shows the project's own value, whatever the application-wide setting it moved from says.
#[test]
fn project_settings_estimates_show_the_projects_value_not_the_application_setting() {
  let mut page = Settings::new(false);
  page.db().set_estimates_story_points(true).expect("on for the project");
  page.tab("general", "settings-project-name");
  page.tab("runs", &format!("{SWITCH}-row"));
  page.until("the project's value shown", |p| p.switch_on(SWITCH));
  assert!(
    !AppDb::open(&page.window.data.database())
      .expect("the application database")
      .estimates_story_points()
      .expect("read"),
    "the application-wide setting is off"
  );
}
