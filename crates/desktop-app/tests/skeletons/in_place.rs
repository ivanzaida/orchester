//! Real content takes the bones' place in the same room, and a section that reads inside a shown page leaves the rest
//! of the page as it is.

use orchester_app::routes::{ProjectPage, ProjectSettingsTab, Route};

use crate::harness::Held;

#[test]
fn skeletons_real_rows_replace_the_bones_without_changing_the_groups_height() {
  let mut held = Held::new();
  held.page(ProjectPage::Settings(ProjectSettingsTab::General));
  held.skeleton_at_once("settings-loading");
  let bones = held.bounds("settings-project-reading");
  held.release("settings-project");
  let read = held.bounds("settings-project");
  assert!(!held.exists("settings-loading"), "the bones are gone");
  assert!(
    (read.y - bones.y).abs() < 0.5,
    "the group stands where its bones stood: {} vs {}",
    read.y,
    bones.y
  );
  assert!(
    (read.height - bones.height).abs() < 0.5,
    "the group is as high as its bones: {} vs {}",
    read.height,
    bones.height
  );
}

#[test]
fn skeletons_a_value_still_read_keeps_the_rest_of_its_page() {
  let mut held = Held::new();
  held.go(Route::General);
  held.skeleton_at_once("settings-threshold-reading");
  assert!(
    held.exists("settings-appearance"),
    "the theme, read already, stays as it is"
  );
  assert!(
    held.exists("settings-threshold-row"),
    "the threshold's row stands round its bone"
  );
  held.release("settings-threshold-value");
  assert!(
    !held.exists("settings-threshold-reading"),
    "the value takes its bone's place"
  );
}

#[test]
fn skeletons_project_settings_tabs_come_up_with_their_groups_and_a_later_read_moves_nothing() {
  let mut held = Held::new();
  held.page(ProjectPage::Settings(ProjectSettingsTab::Context));
  for bones in [
    "settings-sources-loading",
    "settings-memory-loading",
    "settings-sources-reading-title",
    "settings-repository-files-reading-title",
    "settings-index-section-reading-title",
  ] {
    held.until(bones);
  }
  held.page(ProjectPage::Settings(ProjectSettingsTab::Sharing));
  held.skeleton_at_once("project-settings-sharing-loading");
  for real in ["sharing-task-list-reading-label", "sharing-share-reading-label"] {
    assert!(held.exists(real), "{real}: the row's words stay real");
  }
  held.page(ProjectPage::Settings(ProjectSettingsTab::Runs));
  held.skeleton_at_once("settings-loading");
  let bones = held.bounds("settings-limits-reading");
  held.release("settings-limits");
  let read = held.bounds("settings-limits");
  assert!(
    (read.height - bones.height).abs() < 0.5,
    "Limits, read on its own, takes its bones' room: {read:?} vs {bones:?}"
  );
}

#[test]
fn skeletons_a_heading_with_an_action_starts_where_any_other_does() {
  let mut held = Held::new();
  held.page(ProjectPage::Settings(ProjectSettingsTab::General));
  held.until("settings-project-reading-title");
  let plain = (
    held.bounds("settings-project-reading-title"),
    held.bounds("settings-project-reading-rows"),
  );
  held.page(ProjectPage::Connections(Default::default()));
  held.until("connections-add-agent");
  let with_action = (
    held.bounds("connections-reading-agents-title"),
    held.bounds("connections-reading-agents-rows"),
  );
  assert!(
    (plain.0.y - with_action.0.y).abs() < 0.5 && (plain.1.y - with_action.1.y).abs() < 0.5,
    "the same heading and rows' y: {plain:?} vs {with_action:?}"
  );
}
